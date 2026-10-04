import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { createSbom, npmInventory, cargoInventory } from "./generate-sbom.mjs";
const hash = (value) => createHash("sha256").update(value).digest("hex");
const dep = (version, dependencies = {}) => ({ version, path: `/node/${version}`, dependencies });
const npmTree = { dependencies: { runtime: dep("1.0.0", { shared: dep("2.0.0") }) }, devDependencies: { builder: dep("3.0.0", { shared: dep("2.0.0") }) } };
const edge = (pkg, kind = null) => ({ pkg, dep_kinds: [{ kind }] });
const cargo = { packages: [{ id: "root", name: "rice", version: "0.2.3" }, { id: "runtime", name: "lib", version: "1.0.0" }, { id: "build", name: "builder", version: "2.0.0" }], resolve: { root: "root", nodes: [{ id: "root", deps: [edge("runtime"), edge("build", "build")] }, { id: "runtime", deps: [] }, { id: "build", deps: [] }] } };
const lockfiles = { npm: "npm lock", cargo: "cargo lock" };
const commit = "a".repeat(40);
const materials = { schemaVersion: 1, commit, sourceDateEpoch: 1780000000, lockfiles: { npm: hash(lockfiles.npm), cargo: hash(lockfiles.cargo) }, inputs: { rustImage: `rust:1@sha256:${hash("rust")}`, nodeImage: `node:22@sha256:${hash("node")}` }, osPackages: ["zip\t3.0"], tools: { rust: "1.89.0" }, windowsBuildMaterials: [{ path: "sdk/header.h", sha256: hash("header") }], artifacts: [{ name: "Rice.exe", sha256: hash("exe") }, { name: "Rice.zip", sha256: hash("zip") }] };
const args = () => ({ materials: structuredClone(materials), expectedCommit: commit, manifest: { name: "rice", version: "0.2.3" }, npmTree, cargo, lockfiles, artifactHashes: { "Rice.exe": hash("exe"), "Rice.zip": hash("zip") } });
test("npm runtime wins over shared build dependency and preserves transitive edges", () => {
  const { packages, edges } = npmInventory(npmTree);
  assert.equal(packages.get("pkg:npm/shared@2.0.0").scope, "required");
  assert.equal(packages.get("pkg:npm/builder@3.0.0").scope, "excluded");
  assert.ok(edges.get("pkg:npm/runtime@1.0.0").has("pkg:npm/shared@2.0.0"));
});
test("Cargo Windows resolution separates runtime and build dependencies", () => {
  const { scopes } = cargoInventory(cargo);
  assert.equal(scopes.get("runtime"), "required");
  assert.equal(scopes.get("build"), "excluded");
});
test("CycloneDX contains exact commit, artifacts, OS, images and SDK inputs", () => {
  const bom = createSbom(args());
  assert.equal(bom.bomFormat, "CycloneDX");
  assert.equal(bom.metadata.component["bom-ref"], `rice:${commit}`);
  assert.ok(bom.components.some((component) => component.name === "Rice.zip" && component.hashes[0].content === hash("zip")));
  assert.ok(bom.components.some((component) => component.name === "sdk/header.h"));
  const refs = new Set([bom.metadata.component["bom-ref"], ...bom.components.map((component) => component["bom-ref"])]);
  for (const dependency of bom.dependencies) for (const ref of dependency.dependsOn) assert.ok(refs.has(ref));
});
test("wrong source, altered lockfile or artifact and missing build inventory fail", () => {
  for (const mutate of [(input) => input.expectedCommit = "b".repeat(40), (input) => input.lockfiles = { ...lockfiles, npm: "changed" }, (input) => input.artifactHashes["Rice.exe"] = hash("tampered"), (input) => input.materials.osPackages = []]) {
    const input = args(); mutate(input); assert.throws(() => createSbom(input));
  }
});
test("unresolved dependencies cannot produce a misleading complete SBOM", () => {
  const metadata = structuredClone(cargo);
  metadata.resolve.nodes[0].deps.push(edge("absent"));
  assert.throws(() => cargoInventory(metadata));
  assert.throws(() => npmInventory({ dependencies: { bad: {} }, devDependencies: {} }));
});
test("installed project graph produces referentially complete SBOM", { skip: !process.env.RICE_TEST_INSTALLED_GRAPH }, () => {
  const input = args();
  input.npmTree = JSON.parse(execFileSync("pnpm", ["list", "--depth", "Infinity", "--json"], { encoding: "utf8", maxBuffer: 67108864 }))[0];
  input.cargo = JSON.parse(execFileSync("cargo", ["metadata", "--locked", "--manifest-path", "src-tauri/Cargo.toml", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"], { encoding: "utf8", maxBuffer: 67108864 }));
  input.lockfiles = { npm: readFileSync("pnpm-lock.yaml"), cargo: readFileSync("src-tauri/Cargo.lock") };
  input.materials.lockfiles = { npm: hash(input.lockfiles.npm), cargo: hash(input.lockfiles.cargo) };
  const bom = createSbom(input);
  assert.ok(bom.components.length > 400);
  assert.ok(bom.components.some((component) => component.purl?.startsWith("pkg:cargo/tauri@") && component.scope === "required"));
  assert.ok(bom.components.some((component) => component.purl?.startsWith("pkg:npm/braces@") && component.scope === "excluded"));
});
