import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import { verifyReleaseInputs } from "./verify-release-build-inputs.mjs";
import { spawnSync } from "node:child_process";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
test("release inputs agree with the immutable Docker build", () => assert.equal(verifyReleaseInputs(root).schemaVersion, 1));
test("rejects command-line image overrides before accepting runtime versions", () => {
  const result = spawnSync(process.execPath, [join(root, "scripts/verify-release-build-inputs.mjs"), "--runtime"], {
    env: { ...process.env, RICE_BUILD_RUST_IMAGE: "rust:1.89.0-bookworm" }, encoding: "utf8",
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Build argument RUST_IMAGE overrides reviewed inputs/);
});
test("rejects command-line NSIS overrides before accepting runtime versions", () => {
  const result = spawnSync(process.execPath, [join(root, "scripts/verify-release-build-inputs.mjs"), "--runtime"], {
    env: { ...process.env, RICE_BUILD_NSIS_VERSION: "3.08" }, encoding: "utf8",
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Build argument NSIS_VERSION overrides reviewed inputs/);
});
for (const [name, edit] of [
  ["mutable base image", (source) => source.replace(/@sha256:[a-f0-9]{64}/, "")],
  ["snapshot drift", (source) => source.replace("ARG DEBIAN_SNAPSHOT=20260921T000000Z", "ARG DEBIAN_SNAPSHOT=latest")],
  ["unlocked tool installation", (source) => source.replace("--locked", "")],
  ["ZIP metadata regression", (source) => source.replace("zip -X", "zip")],
  ["missing NSIS archive integrity check", source => source.replace("sha256sum --check -", "true")],
  ["missing NSIS packed version metadata", source => source.replace('VER_REVISION=0 VER_BUILD=0', '')],
]) {
  test(`rejects ${name}`, () => {
    const fixture = mkdtempSync(join(tmpdir(), "rice-input-policy-"));
    try {
      mkdirSync(join(fixture, "build"));
      writeFileSync(join(fixture, "build/release-inputs.json"), readFileSync(join(root, "build/release-inputs.json")));
      writeFileSync(join(fixture, "Dockerfile"), edit(readFileSync(join(root, "Dockerfile"), "utf8")));
      assert.throws(() => verifyReleaseInputs(fixture));
    } finally { rmSync(fixture, { recursive: true, force: true }); }
  });
}
for (const [name, key, value] of [
  ["other Windows archive release", "nsisWindowsUrl", "https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.12/nsis-3.12.zip"],
  ["mutable source URL", "nsisSourceUrl", "https://deb.debian.org/debian/pool/main/n/nsis/latest.tar.gz"],
  ["missing strong source digest", "nsisSourceSha256", "1234"],
  ["missing compiler timestamp", "nsisSourceDateEpoch", 0],
]) test(`rejects self-consistent but unreviewed NSIS ${name}`, t => {
  const fixture = mkdtempSync(join(tmpdir(), "rice-nsis-input-policy-"));
  t.after(() => rmSync(fixture, { recursive: true, force: true }));
  mkdirSync(join(fixture, "build"));
  const inputs = JSON.parse(readFileSync(join(root, "build/release-inputs.json")));
  const before = inputs[key]; inputs[key] = value;
  writeFileSync(join(fixture, "build/release-inputs.json"), JSON.stringify(inputs));
  writeFileSync(join(fixture, "Dockerfile"), readFileSync(join(root, "Dockerfile"), "utf8").replace(String(before), String(value)));
  assert.throws(() => verifyReleaseInputs(fixture));
});
