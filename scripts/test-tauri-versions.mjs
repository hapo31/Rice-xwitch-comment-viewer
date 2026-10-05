import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { verifyTauriVersions } from "./verify-tauri-versions.mjs";
const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
test("Rust lock and exact reviewed JS inputs agree", () => assert.equal(verifyTauriVersions(root), "2.12.1"));
for (const [name, edit] of [
  ["old API minor", pkg => pkg.dependencies["@tauri-apps/api"] = "2.11.0"],
  ["old CLI minor", pkg => pkg.devDependencies["@tauri-apps/cli"] = "2.11.2"],
  ["unreviewed floating API", pkg => pkg.dependencies["@tauri-apps/api"] = "^2.12.1"],
  ["other major", pkg => pkg.dependencies["@tauri-apps/api"] = "3.12.1"],
]) test(`rejects ${name} before installer compilation`, t => {
  const fixture = mkdtempSync(join(tmpdir(), "rice-tauri-version-"));
  t.after(() => rmSync(fixture, { recursive: true, force: true }));
  mkdirSync(join(fixture, "src-tauri"));
  const pkg = JSON.parse(readFileSync(join(root, "package.json")));
  edit(pkg); writeFileSync(join(fixture, "package.json"), JSON.stringify(pkg));
  writeFileSync(join(fixture, "src-tauri/Cargo.lock"), readFileSync(join(root, "src-tauri/Cargo.lock")));
  assert.throws(() => verifyTauriVersions(fixture));
});
test("rejects stale installed input even when manifest minor agrees", t => {
  const fixture = mkdtempSync(join(tmpdir(), "rice-tauri-installed-"));
  t.after(() => rmSync(fixture, { recursive: true, force: true }));
  mkdirSync(join(fixture, "src-tauri")); mkdirSync(join(fixture, "node_modules/@tauri-apps/api"), { recursive: true });
  writeFileSync(join(fixture, "package.json"), readFileSync(join(root, "package.json")));
  writeFileSync(join(fixture, "src-tauri/Cargo.lock"), readFileSync(join(root, "src-tauri/Cargo.lock")));
  writeFileSync(join(fixture, "node_modules/@tauri-apps/api/package.json"), JSON.stringify({ version: "2.11.0" }));
  assert.throws(() => verifyTauriVersions(fixture, { installed: true }), /Installed Tauri package/);
});
