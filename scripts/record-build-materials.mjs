import { createHash } from "node:crypto";
import { readFileSync, readdirSync, writeFileSync, createReadStream } from "node:fs";
import { execFileSync } from "node:child_process";
import { join } from "node:path";

const commit = process.env.RICE_GIT_COMMIT;
const epoch = process.env.SOURCE_DATE_EPOCH;
if (!/^[a-f0-9]{40}$/.test(commit ?? "") || !/^\d+$/.test(epoch ?? "") || Number(epoch) < 315532800) {
  throw new Error("Exact source commit and SOURCE_DATE_EPOCH (1980 or later) are required");
}
const hash = (file) => createHash("sha256").update(readFileSync(file)).digest("hex");
const xwinFiles = [];
async function inventory(directory, prefix = "") {
  for (const entry of readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name, "en"))) {
    const relative = join(prefix, entry.name);
    const path = join(directory, entry.name);
    if (entry.isDirectory()) await inventory(path, relative);
    else if (entry.isFile()) {
      const digest = createHash("sha256");
      for await (const chunk of createReadStream(path)) digest.update(chunk);
      xwinFiles.push({ path: relative, sha256: digest.digest("hex") });
    }
  }
}
await inventory(process.env.XWIN_CACHE_DIR);
const inputs = JSON.parse(readFileSync("build/release-inputs.json", "utf8"));
const artifacts = readdirSync("/out").sort().map((name) => ({ name, sha256: hash(join("/out", name)) }));
writeFileSync("/out/BUILD-MATERIALS.json", JSON.stringify({ schemaVersion: 1, commit, sourceDateEpoch: Number(epoch), inputs, lockfiles: { npm: hash("pnpm-lock.yaml"), cargo: hash("src-tauri/Cargo.lock") }, osPackages: execFileSync("dpkg-query", ["-W", "-f=${Package}\t${Version}\n"], { encoding: "utf8" }).trim().split("\n").sort(), tools: { rust: execFileSync("rustc", ["--version"], { encoding: "utf8" }).trim(), node: process.version, cargoXwin: execFileSync("cargo-xwin", ["--version"], { encoding: "utf8" }).trim(), nsis: execFileSync("makensis", ["-VERSION"], { encoding: "utf8" }).trim() }, windowsBuildMaterials: xwinFiles, artifacts }, null, 2) + "\n");
