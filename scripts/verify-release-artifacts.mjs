import { createHash } from "node:crypto";
import { readFileSync, readdirSync, lstatSync, writeFileSync, mkdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { inflateRawSync } from "node:zlib";
import { isDeepStrictEqual } from "node:util";

const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const fail = message => { throw new Error(message); };
const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const MAX_FILE = 512 * 1024 * 1024;
const MAX_ENTRY = 256 * 1024 * 1024;
function read(file, maximum = MAX_FILE) {
  const stat = lstatSync(file);
  if (!stat.isFile() || stat.size < 1 || stat.size > maximum) fail(`Invalid artifact file: ${file}`);
  return readFileSync(file);
}
const json = file => JSON.parse(read(file, 64 * 1024 * 1024));
const same = (actual, expected, message) => { if (!isDeepStrictEqual(actual, expected)) fail(message); };

const CRC_TABLE = Uint32Array.from({ length: 256 }, (_, value) => {
  for (let bit = 0; bit < 8; bit++) value = (value >>> 1) ^ ((value & 1) ? 0xedb88320 : 0);
  return value >>> 0;
});
export function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) crc = (crc >>> 8) ^ CRC_TABLE[(crc ^ byte) & 0xff];
  return (crc ^ 0xffffffff) >>> 0;
}

export function inspectPe(bytes, application = true) {
  if (bytes.length < 0x40 || bytes.toString("ascii", 0, 2) !== "MZ") fail("Missing PE DOS header");
  const pe = bytes.readUInt32LE(0x3c);
  if (pe < 0x40 || pe + 94 > bytes.length || bytes.readUInt32LE(pe) !== 0x4550) fail("Invalid PE header offset/signature");
  const machine = bytes.readUInt16LE(pe + 4);
  const magic = bytes.readUInt16LE(pe + 24);
  const subsystem = bytes.readUInt16LE(pe + 24 + 68);
  if (subsystem !== 2 || (application ? machine !== 0x8664 || magic !== 0x20b : !((machine === 0x14c && magic === 0x10b) || (machine === 0x8664 && magic === 0x20b)))) fail("Wrong PE architecture or GUI subsystem");
  return { machine, magic, subsystem };
}

// Accept the deterministic flat ZIP produced by Docker's zip -X, not arbitrary
// archives. Validate both headers, exact local layout, inflated size and CRC.
export function inspectPortable(bytes, expectedNames = ["rice.exe", "LICENSE"]) {
  if (bytes.length < 22 || bytes.length > MAX_FILE) fail("ZIP size limit");
  let end = -1;
  for (let at = bytes.length - 22; at >= Math.max(0, bytes.length - 65557); at--) {
    if (bytes.readUInt32LE(at) === 0x06054b50 && at + 22 + bytes.readUInt16LE(at + 20) === bytes.length) { end = at; break; }
  }
  if (end < 0) fail("Missing ZIP end record");
  const count = bytes.readUInt16LE(end + 10), centralSize = bytes.readUInt32LE(end + 12), central = bytes.readUInt32LE(end + 16);
  if (bytes.readUInt16LE(end + 4) || bytes.readUInt16LE(end + 6) || bytes.readUInt16LE(end + 8) !== count || count !== expectedNames.length || central + centralSize !== end) fail("ZIP entry count/disk/layout mismatch");
  let at = central, nextLocal = 0;
  const entries = [];
  for (let i = 0; i < count; i++) {
    if (at + 46 > end || bytes.readUInt32LE(at) !== 0x02014b50) fail("Invalid ZIP central header");
    const flags = bytes.readUInt16LE(at + 8), method = bytes.readUInt16LE(at + 10), crc = bytes.readUInt32LE(at + 16);
    const compressed = bytes.readUInt32LE(at + 20), size = bytes.readUInt32LE(at + 24);
    const nameLength = bytes.readUInt16LE(at + 28), extra = bytes.readUInt16LE(at + 30), comment = bytes.readUInt16LE(at + 32), local = bytes.readUInt32LE(at + 42);
    const finish = at + 46 + nameLength + extra + comment;
    if (finish > end || flags & ~0x806 || ![0, 8].includes(method) || size > MAX_ENTRY || local !== nextLocal || bytes.readUInt16LE(at + 34)) fail("Unsupported/oversized ZIP entry");
    const mode = (bytes.readUInt32LE(at + 38) >>> 16) & 0xf000;
    if (mode && mode !== 0x8000) fail("ZIP must contain regular files only");
    const name = bytes.toString("utf8", at + 46, at + 46 + nameLength);
    if (!expectedNames.includes(name) || entries.some(entry => entry.name === name)) fail("Unexpected/duplicate ZIP path");
    if (local + 30 > central || bytes.readUInt32LE(local) !== 0x04034b50) fail("Invalid ZIP local header");
    const localNameLength = bytes.readUInt16LE(local + 26), localExtra = bytes.readUInt16LE(local + 28);
    const data = local + 30 + localNameLength + localExtra;
    if (data + compressed > central || localNameLength !== nameLength || bytes.toString("utf8", local + 30, local + 30 + localNameLength) !== name || bytes.readUInt16LE(local + 6) !== flags || bytes.readUInt16LE(local + 8) !== method || bytes.readUInt32LE(local + 14) !== crc || bytes.readUInt32LE(local + 18) !== compressed || bytes.readUInt32LE(local + 22) !== size) fail("ZIP local/central mismatch");
    const encoded = bytes.subarray(data, data + compressed);
    const content = method === 0 ? encoded : inflateRawSync(encoded, { maxOutputLength: MAX_ENTRY });
    if (content.length !== size || crc32(content) !== crc) fail("ZIP inflated size/CRC mismatch");
    entries.push({ name, size, crc32: crc, sha256: hash(content), content });
    nextLocal = data + compressed;
    at = finish;
  }
  if (at !== end || nextLocal !== central) fail("ZIP hidden/trailing entry data");
  same(entries.map(entry => entry.name).sort(), [...expectedNames].sort(), "ZIP missing files");
  inspectPe(entries.find(entry => entry.name === "rice.exe").content);
  return entries;
}

export function expectations(source, { tag = null, commit } = {}) {
  if (!/^[a-f0-9]{40}$/.test(commit ?? "")) fail("Exact source commit required");
  const pkg = json(join(source, "package.json")), config = json(join(source, "src-tauri/tauri.conf.json"));
  const cargo = read(join(source, "src-tauri/Cargo.toml")).toString().match(/\[package\]([\s\S]*?)(?=\n\[|$)/)?.[1].match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!/^\d+\.\d+\.\d+$/.test(pkg.version) || pkg.version !== config.version || pkg.version !== cargo || config.productName !== "Rice" || (tag !== null && tag !== `v${pkg.version}`)) fail("Manifest/tag/product version mismatch");
  const inputs = json(join(source, "build/release-inputs.json"));
  if (inputs.windowsTarget !== "x86_64-pc-windows-msvc") fail("Unreviewed Windows artifact target");
  return {
    schemaVersion: 1, version: pkg.version, tag, commit, target: inputs.windowsTarget,
    installer: `Rice_${pkg.version}_x64-setup.exe`,
    portable: `Rice_${pkg.version}_${inputs.windowsTarget}_portable.zip`,
    inputs,
  };
}
function coreNames(plan) { return [plan.installer, plan.portable, "LICENSE", "BUILD-MATERIALS.json", "Rice.sbom.cdx.json"].sort(); }
function directoryNames(directory) {
  return readdirSync(directory, { withFileTypes: true }).map(entry => {
    if (!entry.isFile()) fail(`Non-file artifact entry: ${entry.name}`);
    return entry.name;
  }).sort();
}
function inspect(source, directory, options, writing) {
  const plan = expectations(source, options), names = coreNames(plan);
  const all = directoryNames(directory);
  const complete = [...names, "ARTIFACT-MANIFEST.json", "SHA256SUMS.txt"].sort();
  if (writing && isDeepStrictEqual(all, names)) { /* new bundle */ }
  else same(all, complete, "Exact artifact filename/type/count mismatch");
  const entries = inspectPortable(read(join(directory, plan.portable)));
  const license = read(join(source, "LICENSE"));
  same(read(join(directory, "LICENSE")), license, "Bundle LICENSE mismatch");
  same(entries.find(entry => entry.name === "LICENSE").content, license, "Portable LICENSE mismatch");
  const installerPe = inspectPe(read(join(directory, plan.installer)), false);
  const materials = json(join(directory, "BUILD-MATERIALS.json"));
  same(materials.inputs, plan.inputs, "Build input drift");
  if (materials.schemaVersion !== 1 || materials.commit !== plan.commit) fail("Build material source mismatch");
  same(materials.lockfiles, { npm: hash(read(join(source, "pnpm-lock.yaml"))), cargo: hash(read(join(source, "src-tauri/Cargo.lock"))) }, "Lockfile material mismatch");
  const original = [plan.installer, plan.portable, "LICENSE"].sort().map(name => ({ name, sha256: hash(read(join(directory, name))) }));
  same([...materials.artifacts].sort((a, b) => a.name.localeCompare(b.name, "en")), [...original].sort((a, b) => a.name.localeCompare(b.name, "en")), "Build artifact digest mismatch");
  const sbom = json(join(directory, "Rice.sbom.cdx.json"));
  if (sbom.bomFormat !== "CycloneDX" || sbom.metadata?.component?.version !== plan.version) fail("Artifact SBOM version/type mismatch");
  const manifest = {
    schemaVersion: plan.schemaVersion, version: plan.version, tag: plan.tag, commit: plan.commit, target: plan.target,
    installer: plan.installer, portable: plan.portable, installerPe,
    artifacts: names.map(name => { const content = read(join(directory, name)); return { name, size: content.length, sha256: hash(content) }; }),
    portableEntries: entries.map(({ content, ...entry }) => entry),
  };
  return { manifest, entries, complete };
}
export function writeBundle(source, directory, options) {
  const result = inspect(source, directory, options, true);
  writeFileSync(join(directory, "ARTIFACT-MANIFEST.json"), JSON.stringify(result.manifest, null, 2) + "\n");
  const names = result.complete.filter(name => name !== "SHA256SUMS.txt");
  writeFileSync(join(directory, "SHA256SUMS.txt"), names.map(name => `${hash(read(join(directory, name)))}  ${name}\n`).join(""));
  return result.manifest;
}
export function verifyBundle(source, directory, options) {
  const { manifest, entries, complete } = inspect(source, directory, options, false);
  same(json(join(directory, "ARTIFACT-MANIFEST.json")), manifest, "Artifact manifest/source/content mismatch");
  const checksums = complete.filter(name => name !== "SHA256SUMS.txt").map(name => `${hash(read(join(directory, name)))}  ${name}\n`).join("");
  same(read(join(directory, "SHA256SUMS.txt")).toString(), checksums, "Exact checksum list/digest mismatch");
  return { manifest, entries };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2), directory = resolve(args.shift() ?? "release-artifacts");
  const value = flag => { const at = args.indexOf(flag); return at < 0 ? undefined : args[at + 1]; };
  const source = resolve(process.env.RICE_RELEASE_ROOT ?? root);
  const options = { tag: value("--tag") || null, commit: value("--commit") };
  const manifest = args.includes("--write") ? writeBundle(source, directory, options) : verifyBundle(source, directory, options).manifest;
  const extraction = value("--extract");
  if (extraction) {
    const { entries } = verifyBundle(source, directory, options);
    mkdirSync(extraction); // Must be a new, caller-owned destination.
    for (const entry of entries) writeFileSync(join(extraction, entry.name), entry.content, { flag: "wx" });
  }
  console.log(JSON.stringify(manifest));
}
