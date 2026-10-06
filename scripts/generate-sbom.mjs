import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseToml } from "./config-parsers.mjs";
import { Enums, Models, Serialize, Spec } from "@cyclonedx/cyclonedx-library";
import { packagePurl } from "./sbom-purl.mjs";
import { cargoInventory, npmInventory } from "./sbom-inventory.mjs";
import { collectSbomProvenance, checksum, digest } from "./sbom-provenance.mjs";
import { validateCycloneDx15 } from "./sbom-validation.mjs";

export const generatorVersion = "2.0.0";
const property = ([name, value]) => new Models.Property(`rice:${name}`, String(value));

function setProperties(component, entries = []) {
  for (const entry of entries) component.properties.add(property(entry));
}

function addHashes(component, hashes = []) {
  for (const hash of hashes) component.hashes.set(Enums.HashAlgorithm["SHA-256"], hash);
}

function packageComponent({ ref, name, version, purl, scope, hash, license, ecosystem, role }) {
  const component = new Models.Component(Enums.ComponentType.Library, name, {
    bomRef: ref,
    version,
    purl,
    scope: scope === "required" ? Enums.ComponentScope.Required : Enums.ComponentScope.Excluded,
  });
  if (hash) addHashes(component, [hash]);
  if (license) component.licenses.add(new Models.LicenseExpression(license));
  setProperties(component, [["ecosystem", ecosystem], ["role", role]]);
  return component;
}

function provenanceComponent(item) {
  const component = new Models.Component(item.type, item.name, {
    bomRef: item.ref,
    ...(item.version ? { version: item.version } : {}),
    ...(item.purl ? { purl: item.purl } : {}),
    ...(item.scope === "excluded" ? { scope: Enums.ComponentScope.Excluded } : {}),
  });
  addHashes(component, item.hashes);
  setProperties(component, item.properties);
  return component;
}

function cargoLockChecksums(lockfile) {
  const cargoLock = parseToml(lockfile.toString(), "Cargo.lock");
  if (!Array.isArray(cargoLock.package)) throw new Error("Cargo.lock must contain a package list");
  const checksums = new Map();
  for (const item of cargoLock.package) {
    if (typeof item?.name !== "string" || typeof item?.version !== "string") throw new Error("Invalid Cargo.lock package entry");
    if (item.checksum !== undefined && !digest(item.checksum)) throw new Error(`Invalid Cargo registry checksum: ${item.name}`);
    if (item.checksum) checksums.set(`${item.name}@${item.version}`, item.checksum);
  }
  return checksums;
}

export function buildSbomModel({ materials, manifest, npmTree, cargo, lockfiles, artifactHashes, expectedCommit }) {
  const provenance = collectSbomProvenance({ materials, manifest, lockfiles, artifactHashes, expectedCommit });
  const components = new Map();
  const add = (component) => {
    const ref = component.bomRef.toString();
    if (components.has(ref)) throw new Error(`Duplicate SBOM component: ${ref}`);
    components.set(ref, component);
    return component;
  };
  const npm = npmInventory(npmTree), npmRefs = new Map();
  for (const [inventoryRef, item] of npm.packages) {
    const ref = packagePurl("npm", item.name, item.version);
    npmRefs.set(inventoryRef, ref);
    add(packageComponent({ ref, name: item.name, version: item.version, purl: ref, scope: item.scope, ecosystem: "npm", role: item.scope === "required" ? "frontend-runtime" : "build-tool" }));
  }
  for (const [inventoryRef, item] of npm.packages) {
    const component = components.get(npmRefs.get(inventoryRef));
    for (const dependency of npm.edges.get(inventoryRef) ?? []) {
      const target = components.get(npmRefs.get(dependency));
      if (!target) throw new Error(`Unknown npm dependency: ${dependency}`);
      component.dependencies.add(target.bomRef);
    }
  }

  const rust = cargoInventory(cargo), cargoRefs = new Map(cargo.packages.map((pkg) => [pkg.id, packagePurl("cargo", pkg.name, pkg.version)]));
  const checksums = cargoLockChecksums(lockfiles.cargo);
  for (const pkg of cargo.packages) {
    if (pkg.id === cargo.resolve.root) continue;
    const ref = cargoRefs.get(pkg.id), hash = checksums.get(`${pkg.name}@${pkg.version}`);
    if (pkg.source?.startsWith("registry+") && !hash) throw new Error(`Cargo registry checksum missing: ${pkg.name}`);
    add(packageComponent({ ref, name: pkg.name, version: pkg.version, purl: ref, scope: rust.scopes.get(pkg.id) ?? "excluded", hash, license: pkg.license, ecosystem: "cargo", role: rust.scopes.get(pkg.id) === "required" ? "windows-runtime" : "build-dev-or-other-target" }));
  }
  for (const pkg of cargo.packages) {
    if (pkg.id === cargo.resolve.root) continue;
    const component = components.get(cargoRefs.get(pkg.id));
    for (const dependency of rust.edges.get(pkg.id) ?? []) {
      if (dependency === cargo.resolve.root) continue;
      const target = components.get(cargoRefs.get(dependency));
      if (!target) throw new Error(`Unknown Cargo dependency: ${dependency}`);
      component.dependencies.add(target.bomRef);
    }
  }

  for (const item of provenance.components) add(provenanceComponent(item));
  const bom = new Models.Bom({ version: 1 });
  const root = new Models.Component(Enums.ComponentType.Application, provenance.applicationName, {
    bomRef: provenance.rootRef,
    version: provenance.applicationVersion,
  });
  setProperties(root, provenance.rootProperties);
  for (const inventoryRef of npm.roots) {
    const target = components.get(npmRefs.get(inventoryRef));
    if (!target) throw new Error(`Unknown npm root dependency: ${inventoryRef}`);
    root.dependencies.add(target.bomRef);
  }
  for (const dependency of rust.edges.get(cargo.resolve.root) ?? []) {
    const target = components.get(cargoRefs.get(dependency));
    if (!target) throw new Error(`Unknown Cargo root dependency: ${dependency}`);
    root.dependencies.add(target.bomRef);
  }
  for (const ref of provenance.rootRefs) {
    const target = components.get(ref);
    if (!target) throw new Error(`Unknown provenance component: ${ref}`);
    root.dependencies.add(target.bomRef);
  }
  bom.metadata.timestamp = new Date();
  bom.metadata.component = root;
  bom.metadata.tools.tools.add(new Models.Tool({ name: "rice-sbom", version: generatorVersion }));
  for (const component of components.values()) bom.components.add(component);
  return bom;
}

export async function serializeSbom(bom) {
  const serializer = new Serialize.JsonSerializer(new Serialize.JSON.Normalize.Factory(Spec.Spec1dot5));
  const serialized = serializer.serialize(bom, { space: 2, sortLists: true });
  await validateCycloneDx15(serialized);
  return serialized;
}

export async function createSbom(input) {
  return JSON.parse(await serializeSbom(buildSbomModel(input)));
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
  const serialized = await serializeSbom(buildSbomModel({ materials, expectedCommit, manifest: JSON.parse(readFileSync("package.json")), npmTree, cargo, lockfiles: { npm: readFileSync("pnpm-lock.yaml"), cargo: readFileSync("src-tauri/Cargo.lock") }, artifactHashes }));
  const sbom = JSON.parse(serialized);
  writeFileSync(resolve(directory, "Rice.sbom.cdx.json"), `${serialized}\n`);
  console.log(`SBOM: ${sbom.components.length} components; exact commit, artifacts and CycloneDX 1.5 schema verified`);
}
