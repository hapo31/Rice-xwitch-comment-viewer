import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const generatorVersion = "1.0.0";
const property = (name, value) => ({ name: `rice:${name}`, value: String(value) });
const checksum = (content) => createHash("sha256").update(content).digest("hex");
const digest = (value) => /^[a-f0-9]{64}$/.test(value ?? "");
export const packageRef = (ecosystem, name, version) => `pkg:${ecosystem}/${name.replace(/^@/, "%40")}@${encodeURIComponent(version)}`;

export function npmInventory(tree) {
  if (!tree?.dependencies || !tree.devDependencies) throw new Error("Installed npm dependency tree is required");
  const packages = new Map(), edges = new Map(), roots = new Set(), pending = [];
  for (const [scope, entries] of [["required", tree.dependencies], ["excluded", tree.devDependencies]]) {
    for (const [name, entry] of Object.entries(entries)) pending.push({ name, entry, scope, parent: null });
  }
  while (pending.length) {
    const { name, entry, scope, parent } = pending.shift();
    if (!entry.version || !entry.path || !/^\d/.test(entry.version)) throw new Error(`Unresolved npm package: ${name}`);
    const ref = packageRef("npm", name, entry.version);
    if (parent) edges.get(parent).add(ref); else roots.add(ref);
    const previous = packages.get(ref);
    if (previous && (previous.scope === "required" || scope === "excluded")) continue;
    packages.set(ref, { name, version: entry.version, scope, path: entry.path });
    if (!edges.has(ref)) edges.set(ref, new Set());
    for (const [childName, child] of Object.entries(entry.dependencies ?? {})) pending.push({ name: childName, entry: child, scope, parent: ref });
  }
  return { packages, edges, roots };
}

export function cargoInventory(metadata) {
  if (!metadata.resolve?.root || !Array.isArray(metadata.resolve.nodes) || !Array.isArray(metadata.packages)) throw new Error("Resolved Windows Cargo metadata is required");
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const scopes = new Map(), edges = new Map(), pending = [[metadata.resolve.root, "required"]];
  while (pending.length) {
    const [id, scope] = pending.shift();
    if (scopes.get(id) === "required" || scopes.get(id) === scope) continue;
    scopes.set(id, scope);
    const node = nodes.get(id);
    if (!node) throw new Error(`Missing Cargo dependency node: ${id}`);
    const children = [];
    for (const dependency of node.deps) {
      const kinds = dependency.dep_kinds.filter((kind) => kind.kind !== "dev");
      if (!kinds.length) continue;
      const childScope = scope === "required" && kinds.some((kind) => kind.kind === null) ? "required" : "excluded";
      children.push(dependency.pkg); pending.push([dependency.pkg, childScope]);
    }
    edges.set(id, new Set(children));
  }
  return { scopes, edges };
}

export function createSbom({ materials, manifest, npmTree, cargo, lockfiles, artifactHashes, expectedCommit }) {
  if (materials.schemaVersion !== 1 || !/^[a-f0-9]{40}$/.test(materials.commit ?? "") || materials.commit !== expectedCommit || !Number.isInteger(materials.sourceDateEpoch)) throw new Error("Build materials must match the exact source commit");
  for (const kind of ["npm", "cargo"]) if (!digest(materials.lockfiles?.[kind]) || materials.lockfiles[kind] !== checksum(lockfiles[kind])) throw new Error(`Build lockfile mismatch: ${kind}`);
  if (!Array.isArray(materials.artifacts) || !materials.artifacts.some((artifact) => artifact.name.endsWith(".exe")) || !materials.artifacts.some((artifact) => artifact.name.endsWith(".zip"))) throw new Error("Installer and portable artifacts are required");
  if (!Array.isArray(materials.osPackages) || !materials.osPackages.length || !Array.isArray(materials.windowsBuildMaterials) || !materials.windowsBuildMaterials.length) throw new Error("OS and Windows build material inventories are required");
  const components = new Map(), edges = new Map(), rootEdges = new Set();
  const rootRef = `rice:${materials.commit}`;
  const add = (component) => {
    const ref = component["bom-ref"];
    if (components.has(ref)) throw new Error(`Duplicate SBOM component: ${ref}`);
    components.set(ref, component); edges.set(ref, new Set());
  };
  const npm = npmInventory(npmTree);
  for (const [ref, item] of npm.packages) {
    add({ type: "library", "bom-ref": ref, name: item.name, version: item.version, purl: ref, scope: item.scope, properties: [property("ecosystem", "npm"), property("role", item.scope === "required" ? "frontend-runtime" : "build-tool")] });
    edges.set(ref, npm.edges.get(ref));
  }
  for (const ref of npm.roots) rootEdges.add(ref);
  const rust = cargoInventory(cargo);
  const rustRefs = new Map(cargo.packages.map((pkg) => [pkg.id, packageRef("cargo", pkg.name, pkg.version)]));
  const lockChecksums = new Map();
  for (const section of lockfiles.cargo.toString().split("[[package]]").slice(1)) {
    const name = section.match(/^name = "([^"]+)"/m)?.[1], version = section.match(/^version = "([^"]+)"/m)?.[1];
    const hash = section.match(/^checksum = "([a-f0-9]{64})"/m)?.[1];
    if (hash) lockChecksums.set(`${name}@${version}`, hash);
  }
  for (const pkg of cargo.packages) {
    if (pkg.id === cargo.resolve.root) continue;
    const ref = rustRefs.get(pkg.id), hash = lockChecksums.get(`${pkg.name}@${pkg.version}`);
    if (pkg.source?.startsWith("registry+") && !hash) throw new Error(`Cargo registry checksum missing: ${pkg.name}`);
    const scope = rust.scopes.get(pkg.id) ?? "excluded";
    add({ type: "library", "bom-ref": ref, name: pkg.name, version: pkg.version, purl: ref, scope, ...(hash ? { hashes: [{ alg: "SHA-256", content: hash }] } : {}), ...(pkg.license ? { licenses: [{ expression: pkg.license }] } : {}), properties: [property("ecosystem", "cargo"), property("role", scope === "required" ? "windows-runtime" : "build-dev-or-other-target")] });
    edges.set(ref, new Set([...(rust.edges.get(pkg.id) ?? [])].filter((id) => id !== cargo.resolve.root).map((id) => rustRefs.get(id))));
  }
  for (const id of rust.edges.get(cargo.resolve.root)) rootEdges.add(rustRefs.get(id));
  for (const image of [materials.inputs?.rustImage, materials.inputs?.nodeImage]) {
    if (!/@sha256:[a-f0-9]{64}$/.test(image ?? "")) throw new Error("Build image digest missing");
    const ref = `build-image:${image}`;
    add({ type: "container", "bom-ref": ref, name: image.split("@")[0], scope: "excluded", hashes: [{ alg: "SHA-256", content: image.split("@sha256:")[1] }], properties: [property("role", "build-image")] }); rootEdges.add(ref);
  }
  for (const line of materials.osPackages) {
    const [name, version] = line.split("\t");
    if (!name || !version) throw new Error("Invalid installed OS package inventory");
    const ref = `pkg:deb/debian/${encodeURIComponent(name)}@${encodeURIComponent(version)}`;
    add({ type: "library", "bom-ref": ref, name, version, purl: ref, scope: "excluded", properties: [property("role", "build-os-package")] }); rootEdges.add(ref);
  }
  for (const name of Object.keys(materials.toolHashes ?? {})) {
    if (!Object.hasOwn(materials.tools ?? {}, name) || !digest(materials.toolHashes[name])) throw new Error("Unknown tool or invalid compiler hash");
  }
  if (materials.inputs?.nsisVersion && (!digest(materials.toolHashes?.nsis) || materials.tools?.nsis !== `v${materials.inputs.nsisVersion}`)) throw new Error("Reviewed NSIS compiler version and digest are required");
  for (const [name, version] of Object.entries(materials.tools ?? {})) {
    const ref = `build-tool:${name}`;
    const sha256 = materials.toolHashes?.[name];
    add({ type: "application", "bom-ref": ref, name, version, scope: "excluded", ...(sha256 ? { hashes: [{ alg: "SHA-256", content: sha256 }] } : {}), properties: [property("role", "build-tool")] }); rootEdges.add(ref);
  }
  for (const file of materials.windowsBuildMaterials) {
    if (!digest(file.sha256)) throw new Error("Windows material hash missing");
    const ref = `windows-material:${file.path}`;
    add({ type: "file", "bom-ref": ref, name: file.path, scope: "excluded", hashes: [{ alg: "SHA-256", content: file.sha256 }], properties: [property("role", "windows-sdk-crt-build-input")] }); rootEdges.add(ref);
  }
  for (const artifact of materials.artifacts) {
    if (basename(artifact.name) !== artifact.name || !digest(artifact.sha256) || artifactHashes[artifact.name] !== artifact.sha256) throw new Error(`Artifact digest mismatch: ${artifact.name}`);
    const ref = `artifact:${artifact.name}`;
    add({ type: "file", "bom-ref": ref, name: artifact.name, hashes: [{ alg: "SHA-256", content: artifact.sha256 }] }); rootEdges.add(ref);
  }
  edges.set(rootRef, rootEdges);
  for (const dependencies of edges.values()) for (const ref of dependencies) if (!components.has(ref) && ref !== rootRef) throw new Error(`Unknown SBOM dependency: ${ref}`);
  return { bomFormat: "CycloneDX", specVersion: "1.5", version: 1, metadata: { timestamp: new Date().toISOString(), tools: [{ name: "rice-sbom", version: generatorVersion }], component: { type: "application", "bom-ref": rootRef, name: manifest.name, version: manifest.version, properties: [property("source-commit", materials.commit), property("source-date-epoch", materials.sourceDateEpoch), property("npm-lock-sha256", materials.lockfiles.npm), property("cargo-lock-sha256", materials.lockfiles.cargo)] } }, components: [...components.values()].sort((a, b) => a["bom-ref"].localeCompare(b["bom-ref"], "en")), dependencies: [...edges].sort(([a], [b]) => a.localeCompare(b, "en")).map(([ref, dependsOn]) => ({ ref, dependsOn: [...dependsOn].sort() })) };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const directory = resolve(process.argv[2] ?? "release-artifacts");
  const materials = JSON.parse(readFileSync(resolve(directory, "BUILD-MATERIALS.json")));
  const expectedCommit = process.env.GITHUB_SHA ?? execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  const npmTree = JSON.parse(execFileSync("pnpm", ["list", "--depth", "Infinity", "--json"], { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 }))[0];
  const cargo = JSON.parse(execFileSync("cargo", ["metadata", "--locked", "--manifest-path", "src-tauri/Cargo.toml", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"], { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 }));
  const artifactHashes = Object.fromEntries(materials.artifacts.map((artifact) => {
    if (basename(artifact.name) !== artifact.name) throw new Error("Unsafe artifact name");
    return [artifact.name, checksum(readFileSync(resolve(directory, artifact.name)))];
  }));
  const sbom = createSbom({ materials, expectedCommit, manifest: JSON.parse(readFileSync("package.json")), npmTree, cargo, lockfiles: { npm: readFileSync("pnpm-lock.yaml"), cargo: readFileSync("src-tauri/Cargo.lock") }, artifactHashes });
  writeFileSync(resolve(directory, "Rice.sbom.cdx.json"), JSON.stringify(sbom, null, 2) + "\n");
  console.log(`SBOM: ${sbom.components.length} components; exact commit and artifact hashes verified`);
}
