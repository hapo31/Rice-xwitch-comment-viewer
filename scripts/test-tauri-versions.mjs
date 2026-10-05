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
  writeFileSync(join(fixture, "pnpm-lock.yaml"), readFileSync(join(root, "pnpm-lock.yaml")));
  assert.throws(() => verifyTauriVersions(fixture));
});
test("rejects stale installed input even when manifest minor agrees", t => {
  const fixture = mkdtempSync(join(tmpdir(), "rice-tauri-installed-"));
  t.after(() => rmSync(fixture, { recursive: true, force: true }));
  mkdirSync(join(fixture, "src-tauri")); mkdirSync(join(fixture, "node_modules/@tauri-apps/api"), { recursive: true });
  writeFileSync(join(fixture, "package.json"), readFileSync(join(root, "package.json")));
  writeFileSync(join(fixture, "src-tauri/Cargo.lock"), readFileSync(join(root, "src-tauri/Cargo.lock")));
  writeFileSync(join(fixture, "pnpm-lock.yaml"), readFileSync(join(root, "pnpm-lock.yaml")));
  writeFileSync(join(fixture, "node_modules/@tauri-apps/api/package.json"), JSON.stringify({ version: "2.11.0" }));
  assert.throws(() => verifyTauriVersions(fixture, { installed: true }), /Installed Tauri package/);
});

function pluginFixture(t) {
  const fixture = mkdtempSync(join(tmpdir(), "rice-tauri-plugin-"));
  t.after(() => rmSync(fixture, { recursive: true, force: true }));
  mkdirSync(join(fixture, "src-tauri"));
  for (const name of ["package.json", "pnpm-lock.yaml", "src-tauri/Cargo.lock"]) {
    writeFileSync(join(fixture, name), readFileSync(join(root, name)));
  }
  return fixture;
}

test("rejects the previous Rust dialog minor before native compilation", t => {
  const fixture = pluginFixture(t);
  const path = join(fixture, "src-tauri/Cargo.lock");
  writeFileSync(path, readFileSync(path, "utf8").replace(/(name = "tauri-plugin-dialog"\nversion = ")[^"]+/, (_, prefix) => `${prefix}2.7.2`));
  assert.throws(() => verifyTauriVersions(fixture), /major-minor mismatch: @tauri-apps\/plugin-dialog/);
});

test("allows dialog patch differences within the same Rust/JS minor", t => {
  const fixture = pluginFixture(t);
  const path = join(fixture, "src-tauri/Cargo.lock");
  writeFileSync(path, readFileSync(path, "utf8").replace(/(name = "tauri-plugin-dialog"\nversion = ")[^"]+/, (_, prefix) => `${prefix}2.8.2`));
  assert.equal(verifyTauriVersions(fixture), "2.12.1");
});

test("rejects a missing Rust counterpart for an imported JS plugin", t => {
  const fixture = pluginFixture(t);
  const path = join(fixture, "src-tauri/Cargo.lock");
  writeFileSync(path, readFileSync(path, "utf8").replace('name = "tauri-plugin-dialog"', 'name = "missing-dialog"'));
  assert.throws(() => verifyTauriVersions(fixture), /Expected exactly one locked Tauri plugin/);
});

test("rejects ambiguous locked Rust plugin versions", t => {
  const fixture = pluginFixture(t);
  const path = join(fixture, "src-tauri/Cargo.lock");
  writeFileSync(path, `${readFileSync(path, "utf8")}\n[[package]]\nname = "tauri-plugin-dialog"\nversion = "2.8.2"\n`);
  assert.throws(() => verifyTauriVersions(fixture), /Expected exactly one locked Tauri plugin/);
});

test("rejects a JS plugin missing from the package-manager lock", t => {
  const fixture = pluginFixture(t);
  const path = join(fixture, "pnpm-lock.yaml");
  writeFileSync(path, readFileSync(path, "utf8").replace("  '@tauri-apps/plugin-dialog':", "  'missing-plugin':"));
  assert.throws(() => verifyTauriVersions(fixture), /Missing locked JS Tauri plugin/);
});

test("rejects a stale installed plugin even when both locked minors agree", t => {
  const fixture = pluginFixture(t);
  const pkg = JSON.parse(readFileSync(join(fixture, "package.json")));
  for (const [name, version] of [
    ["@tauri-apps/api", pkg.dependencies["@tauri-apps/api"]],
    ["@tauri-apps/cli", pkg.devDependencies["@tauri-apps/cli"]],
    ["@tauri-apps/plugin-dialog", "2.7.2"],
  ]) {
    mkdirSync(join(fixture, "node_modules", name), { recursive: true });
    writeFileSync(join(fixture, "node_modules", name, "package.json"), JSON.stringify({ version }));
  }
  assert.throws(() => verifyTauriVersions(fixture, { installed: true }), /Installed Tauri package.*plugin-dialog/);
});
