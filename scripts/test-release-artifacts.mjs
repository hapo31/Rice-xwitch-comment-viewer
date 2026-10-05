import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync, renameSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { deflateRawSync } from "node:zlib";
import { crc32, inspectPe, inspectPortable, expectations, writeBundle, verifyBundle } from "./verify-release-artifacts.mjs";

const digest = bytes => createHash("sha256").update(bytes).digest("hex");
const commit = "a".repeat(40);
function pe() {
  const bytes = Buffer.alloc(256);
  bytes.write("MZ"); bytes.writeUInt32LE(64, 0x3c); bytes.writeUInt32LE(0x4550, 64);
  bytes.writeUInt16LE(0x8664, 68); bytes.writeUInt16LE(0x20b, 88); bytes.writeUInt16LE(2, 156);
  return bytes;
}
function zip(files = [["rice.exe", pe()], ["LICENSE", Buffer.from("test-license\n")]], compressed = true) {
  const locals = [], centrals = []; let offset = 0;
  for (const [name, content] of files) {
    const encodedName = Buffer.from(name), packed = compressed ? deflateRawSync(content, { level: 9 }) : content;
    const flags = compressed ? 2 : 0, method = compressed ? 8 : 0;
    const local = Buffer.alloc(30), central = Buffer.alloc(46);
    local.writeUInt32LE(0x04034b50); local.writeUInt16LE(flags, 6); local.writeUInt16LE(method, 8);
    local.writeUInt32LE(crc32(content), 14); local.writeUInt32LE(packed.length, 18); local.writeUInt32LE(content.length, 22); local.writeUInt16LE(encodedName.length, 26);
    central.writeUInt32LE(0x02014b50); central.writeUInt16LE(flags, 8); central.writeUInt16LE(method, 10);
    central.writeUInt32LE(crc32(content), 16); central.writeUInt32LE(packed.length, 20); central.writeUInt32LE(content.length, 24); central.writeUInt16LE(encodedName.length, 28); central.writeUInt32LE(offset, 42);
    locals.push(local, encodedName, packed); centrals.push(central, encodedName);
    offset += local.length + encodedName.length + packed.length;
  }
  const central = Buffer.concat(centrals), end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50); end.writeUInt16LE(files.length, 8); end.writeUInt16LE(files.length, 10); end.writeUInt32LE(central.length, 12); end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, central, end]);
}
function fixture(t) {
  const source = mkdtempSync(join(tmpdir(), "rice-artifact-policy-"));
  t.after(() => rmSync(source, { recursive: true, force: true }));
  mkdirSync(join(source, "src-tauri")); mkdirSync(join(source, "build")); mkdirSync(join(source, "artifacts"));
  const inputs = JSON.parse(readFileSync(new URL("../build/release-inputs.json", import.meta.url)));
  writeFileSync(join(source, "package.json"), JSON.stringify({ name: "rice", version: "0.2.3" }));
  writeFileSync(join(source, "src-tauri/tauri.conf.json"), JSON.stringify({ productName: "Rice", version: "0.2.3" }));
  writeFileSync(join(source, "src-tauri/Cargo.toml"), '[package]\nname = "rice"\nversion = "0.2.3"\n');
  writeFileSync(join(source, "build/release-inputs.json"), JSON.stringify(inputs));
  writeFileSync(join(source, "pnpm-lock.yaml"), "fake-lock"); writeFileSync(join(source, "src-tauri/Cargo.lock"), "fake-cargo-lock");
  writeFileSync(join(source, "LICENSE"), "test-license\n");
  const options = { commit, tag: "v0.2.3" }, plan = expectations(source, options), directory = join(source, "artifacts");
  writeFileSync(join(directory, plan.installer), pe()); writeFileSync(join(directory, plan.portable), zip()); writeFileSync(join(directory, "LICENSE"), "test-license\n");
  writeFileSync(join(directory, "BUILD-MATERIALS.json"), JSON.stringify({ schemaVersion: 1, commit, inputs, lockfiles: { npm: digest(Buffer.from("fake-lock")), cargo: digest(Buffer.from("fake-cargo-lock")) }, artifacts: [plan.installer, plan.portable, "LICENSE"].map(name => ({ name, sha256: digest(readFileSync(join(directory, name))) })) }));
  writeFileSync(join(directory, "Rice.sbom.cdx.json"), JSON.stringify({ bomFormat: "CycloneDX", metadata: { component: { version: "0.2.3" } } }));
  return { source, directory, options, plan };
}
test("CRC has a known independent standard vector", () => assert.equal(crc32(Buffer.from("123456789")), 0xcbf43926));
test("checks both stored and max-compression ZIP entries and PE structure", () => {
  for (const compressed of [false, true]) assert.deepEqual(inspectPortable(zip(undefined, compressed)).map(x => x.name), ["rice.exe", "LICENSE"]);
});
test("writes and verifies an exact source-bound bundle (synthetic structural fixture only)", t => {
  const f = fixture(t), manifest = writeBundle(f.source, f.directory, f.options);
  assert.deepEqual(verifyBundle(f.source, f.directory, f.options).manifest, manifest);
  assert.equal(manifest.artifacts.length, 5); assert.equal(manifest.portableEntries.length, 2);
  assert.throws(() => verifyBundle(f.source, f.directory, { ...f.options, commit: "b".repeat(40) }), /source mismatch/);
  assert.throws(() => verifyBundle(f.source, f.directory, { ...f.options, tag: "v9.9.9" }), /version mismatch/);
});
for (const [name, mutate] of [
  ["extra executable", f => writeFileSync(join(f.directory, "extra.exe"), pe())],
  ["missing portable", f => rmSync(join(f.directory, f.plan.portable))],
  ["wrong installer name/version", f => renameSync(join(f.directory, f.plan.installer), join(f.directory, "Rice_0.2.2_x64-setup.exe"))],
  ["unknown nested directory", f => mkdirSync(join(f.directory, "other"))],
  ["license drift", f => writeFileSync(join(f.directory, "LICENSE"), "different")],
  ["manifest drift", f => { const file = join(f.directory, "ARTIFACT-MANIFEST.json"), value = JSON.parse(readFileSync(file)); value.version = "9.9.9"; writeFileSync(file, JSON.stringify(value)); }],
  ["checksum omission", f => writeFileSync(join(f.directory, "SHA256SUMS.txt"), "")],
  ["lockfile drift", f => writeFileSync(join(f.source, "pnpm-lock.yaml"), "changed")],
]) test(`rejects ${name}`, t => { const f = fixture(t); writeBundle(f.source, f.directory, f.options); mutate(f); assert.throws(() => verifyBundle(f.source, f.directory, f.options)); });
for (const [name, files] of [
  ["missing exe", [["LICENSE", Buffer.from("test-license\n")]]],
  ["extra file", [["rice.exe", pe()], ["LICENSE", Buffer.from("x")], ["evil.txt", Buffer.from("x")]]],
  ["duplicate exe", [["rice.exe", pe()], ["rice.exe", pe()]]],
  ["traversal", [["../rice.exe", pe()], ["LICENSE", Buffer.from("x")]]],
]) test(`rejects ZIP ${name}`, () => assert.throws(() => inspectPortable(zip(files))));
test("rejects CRC corruption even when entry headers and PE signature still agree", () => {
  const bytes = zip(undefined, false); bytes[30 + "rice.exe".length + 200] ^= 1;
  assert.throws(() => inspectPortable(bytes), /CRC mismatch/);
});
test("rejects ZIP local/central disagreement, truncation, hidden data and inflated size limit", () => {
  const original = zip(), central = original.readUInt32LE(original.length - 6);
  const badLocal = Buffer.from(original); badLocal[14] ^= 1;
  assert.throws(() => inspectPortable(badLocal), /local\/central mismatch/);
  const large = Buffer.from(original); large.writeUInt32LE(300 * 1024 * 1024, central + 24);
  assert.throws(() => inspectPortable(large), /oversized/);
  assert.throws(() => inspectPortable(original.subarray(0, original.length - 1)));
  assert.throws(() => inspectPortable(Buffer.concat([Buffer.from("hidden"), original])));
});
test("rejects a fake PE, wrong machine and a console subsystem", () => {
  assert.throws(() => inspectPe(Buffer.from("not-exe")));
  for (const [offset, value] of [[68, 0xaa64], [156, 3]]) { const bytes = pe(); bytes.writeUInt16LE(value, offset); assert.throws(() => inspectPe(bytes)); }
});
