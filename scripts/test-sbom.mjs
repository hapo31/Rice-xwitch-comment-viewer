import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { createSbom, generatorVersion } from "./generate-sbom.mjs";
import { cargoInventory, npmInventory } from "./sbom-inventory.mjs";
import { packagePurl, parsePackagePurl } from "./sbom-purl.mjs";
import { validateCycloneDx15 } from "./sbom-validation.mjs";

const hash = (value) => createHash("sha256").update(value).digest("hex");
const dep = (version, dependencies = {}) => ({ version, path: `/node/${version}`, dependencies });
const npmTree = { dependencies: { runtime: dep("1.0.0", { shared: dep("2.0.0") }), "@scope/core": dep("4.0.0") }, devDependencies: { builder: dep("3.0.0", { shared: dep("2.0.0") }) } };
const edge = (pkg, kind = null) => ({ pkg, dep_kinds: [{ kind }] });
const cargo = { packages: [{ id: "root", name: "rice", version: "0.2.3" }, { id: "runtime", name: "lib", version: "1.0.0", source: "registry+https://github.com/rust-lang/crates.io-index" }, { id: "build", name: "builder", version: "2.0.0" }], resolve: { root: "root", nodes: [{ id: "root", deps: [edge("runtime"), edge("build", "build")] }, { id: "runtime", deps: [] }, { id: "build", deps: [] }] } };
const lockfiles = { npm: "npm lock", cargo: `version = 4\n\n[[package]]\nname = "lib"\nversion = "1.0.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "${hash("registry crate")}"\n\n[[package]]\nname = "builder"\nversion = "2.0.0"\n` };
const commit = "a".repeat(40);
const materials = { schemaVersion: 1, commit, sourceDateEpoch: 1780000000, lockfiles: { npm: hash(lockfiles.npm), cargo: hash(lockfiles.cargo) }, inputs: { rustImage: `rust:1@sha256:${hash("rust")}`, nodeImage: `node:22@sha256:${hash("node")}` }, osPackages: ["zip\t3.0"], tools: { rust: "1.89.0" }, windowsBuildMaterials: [{ path: "sdk/header.h", sha256: hash("header") }], artifacts: [{ name: "Rice.exe", sha256: hash("exe") }, { name: "Rice.zip", sha256: hash("zip") }] };
const args = () => ({ materials: structuredClone(materials), expectedCommit: commit, manifest: { name: "rice", version: "0.2.3" }, npmTree, cargo: structuredClone(cargo), lockfiles: { ...lockfiles }, artifactHashes: { "Rice.exe": hash("exe"), "Rice.zip": hash("zip") } });

test("PackageURL builder/parser round-trips npm scoped, Cargo and Debian coordinates", () => {
  for (const [type, name, version, namespace, expected] of [
    ["npm", "@scope/core", "4.0.0", undefined, "pkg:npm/%40scope/core@4.0.0"],
    ["cargo", "serde+derive", "1.0.0", undefined, "pkg:cargo/serde%2Bderive@1.0.0"],
    ["deb", "zip", "3:3.0-1", "debian", "pkg:deb/debian/zip@3:3.0-1"],
  ]) {
    const purl = packagePurl(type, name, version, namespace);
    assert.equal(purl, expected);
    assert.equal(parsePackagePurl(purl).toString(), purl);
  }
  assert.throws(() => packagePurl("npm", "@scope", "1.0.0"), /Invalid scoped npm/);
});

test("npm runtime wins over shared build dependency and preserves transitive edges", () => {
  const { packages, edges } = npmInventory(npmTree);
  assert.equal(packages.get("shared@2.0.0").scope, "required");
  assert.equal(packages.get("builder@3.0.0").scope, "excluded");
  assert.ok(edges.get("runtime@1.0.0").has("shared@2.0.0"));
});

test("Cargo Windows resolution separates runtime and build dependencies", () => {
  const { scopes } = cargoInventory(cargo);
  assert.equal(scopes.get("runtime"), "required");
  assert.equal(scopes.get("build"), "excluded");
});

test("CycloneDX model preserves exact provenance, dependency graph and stable serialized components", async () => {
  const bom = await createSbom(args());
  assert.equal(bom.$schema, "http://cyclonedx.org/schema/bom-1.5.schema.json");
  assert.equal(bom.bomFormat, "CycloneDX");
  assert.equal(bom.specVersion, "1.5");
  assert.equal(bom.metadata.component["bom-ref"], `rice:${commit}`);
  assert.ok(bom.metadata.component.properties.some((entry) => entry.name === "rice:source-commit" && entry.value === commit));
  assert.ok(bom.metadata.component.properties.some((entry) => entry.name === "rice:npm-lock-sha256" && entry.value === hash(lockfiles.npm)));
  assert.equal(bom.metadata.tools[0].name, "rice-sbom");
  assert.equal(bom.metadata.tools[0].version, generatorVersion);
  assert.deepEqual(bom.components.map((component) => component["bom-ref"]), [...bom.components.map((component) => component["bom-ref"])].sort((a, b) => a.localeCompare(b, "en")));
  assert.ok(bom.components.some((component) => component.name === "Rice.zip" && component.hashes[0].content === hash("zip")));
  assert.ok(bom.components.some((component) => component.name === "sdk/header.h" && component.scope === "excluded"));
  assert.ok(bom.components.some((component) => component.purl === "pkg:deb/debian/zip@3.0" && component.scope === "excluded"));
  assert.ok(bom.components.some((component) => component.purl === "pkg:npm/%40scope/core@4.0.0"));
  assert.ok(bom.components.some((component) => component.purl === "pkg:cargo/lib@1.0.0" && component.hashes?.[0]?.content === hash("registry crate")));
  const refs = new Set([bom.metadata.component["bom-ref"], ...bom.components.map((component) => component["bom-ref"])]);
  for (const dependency of bom.dependencies) {
    assert.ok(refs.has(dependency.ref));
    for (const ref of dependency.dependsOn ?? []) assert.ok(refs.has(ref));
  }
  const dependencyRefs = bom.dependencies.map((dependency) => dependency.ref);
  assert.deepEqual(dependencyRefs, [...dependencyRefs].sort((a, b) => a.localeCompare(b, "en")));
  for (const dependency of bom.dependencies) {
    const dependencies = dependency.dependsOn ?? [];
    assert.deepEqual(dependencies, [...dependencies].sort((a, b) => a.localeCompare(b, "en")));
  }
  const rootDependencies = bom.dependencies.find((dependency) => dependency.ref === `rice:${commit}`).dependsOn;
  assert.ok(rootDependencies.includes("pkg:npm/%40scope/core@4.0.0"));
  assert.ok(rootDependencies.includes("pkg:deb/debian/zip@3.0"));
});

test("CycloneDX official 1.5 schema validator rejects malformed documents", async () => {
  await assert.rejects(validateCycloneDx15(JSON.stringify({ bomFormat: "CycloneDX" })), /CycloneDX 1\.5 schema validation failed/);
});

test("wrong source, altered lockfile or artifact and missing build inventory fail", async () => {
  for (const mutate of [(input) => input.expectedCommit = "b".repeat(40), (input) => input.lockfiles = { ...lockfiles, npm: "changed" }, (input) => input.artifactHashes["Rice.exe"] = hash("tampered"), (input) => input.materials.osPackages = []]) {
    const input = args(); mutate(input); await assert.rejects(createSbom(input));
  }
});

test("malformed Cargo lockfiles and invalid registry checksums fail closed", async () => {
  const invalidToml = args();
  invalidToml.lockfiles.cargo = "[[package]\nname = \"lib\"";
  invalidToml.materials.lockfiles.cargo = hash(invalidToml.lockfiles.cargo);
  await assert.rejects(createSbom(invalidToml));

  const invalidChecksum = args();
  invalidChecksum.lockfiles.cargo = invalidChecksum.lockfiles.cargo.replace(hash("registry crate"), `${"0".repeat(63)}x`);
  invalidChecksum.materials.lockfiles.cargo = hash(invalidChecksum.lockfiles.cargo);
  invalidChecksum.cargo.packages[1].source = "registry+https://github.com/rust-lang/crates.io-index";
  await assert.rejects(createSbom(invalidChecksum), /Invalid Cargo registry checksum/);
});

test("native compiler hash belongs to the NSIS tool, not a fictitious extra component", async () => {
  const input = args();
  input.materials.inputs.nsisVersion = "3.11";
  input.materials.tools.nsis = "v3.11";
  input.materials.toolHashes = { nsis: hash("native compiler") };
  const bom = await createSbom(input);
  const nsis = bom.components.find(component => component["bom-ref"] === "build-tool:nsis");
  assert.equal(nsis.version, "v3.11");
  assert.deepEqual(nsis.hashes, [{ alg: "SHA-256", content: hash("native compiler") }]);
  assert.ok(!bom.components.some(component => component.name === "nsisCompilerSha256"));
  for (const mutate of [
    value => delete value.materials.toolHashes.nsis,
    value => value.materials.toolHashes.nsis = "invalid",
    value => value.materials.toolHashes.unknown = hash("unknown"),
    value => value.materials.toolHashes.toString = hash("inherited name"),
    value => value.materials.tools.nsis = "v3.08",
  ]) {
    const wrong = structuredClone(input); mutate(wrong);
    await assert.rejects(createSbom(wrong));
  }
});

test("unresolved dependencies cannot produce a misleading complete SBOM", () => {
  const metadata = structuredClone(cargo);
  metadata.resolve.nodes[0].deps.push(edge("absent"));
  assert.throws(() => cargoInventory(metadata));
  assert.throws(() => npmInventory({ dependencies: { bad: {} }, devDependencies: {} }));
});

test("installed project graph produces schema-valid referentially complete SBOM", { skip: !process.env.RICE_TEST_INSTALLED_GRAPH }, async () => {
  const input = args();
  input.npmTree = JSON.parse(execFileSync("pnpm", ["list", "--depth", "Infinity", "--json"], { encoding: "utf8", maxBuffer: 67108864 }))[0];
  input.cargo = JSON.parse(execFileSync("cargo", ["metadata", "--locked", "--manifest-path", "src-tauri/Cargo.toml", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"], { encoding: "utf8", maxBuffer: 67108864 }));
  input.lockfiles = { npm: readFileSync("pnpm-lock.yaml"), cargo: readFileSync("src-tauri/Cargo.lock") };
  input.materials.lockfiles = { npm: hash(input.lockfiles.npm), cargo: hash(input.lockfiles.cargo) };
  const bom = await createSbom(input);
  assert.ok(bom.components.length > 400);
  assert.ok(bom.components.some((component) => component.purl?.startsWith("pkg:cargo/tauri@") && component.scope === "required"));
  assert.ok(bom.components.some((component) => component.purl?.startsWith("pkg:npm/tailwindcss@") && component.scope === "excluded"));
  assert.ok(!bom.components.some((component) => component.purl?.startsWith("pkg:npm/braces@")));
});
