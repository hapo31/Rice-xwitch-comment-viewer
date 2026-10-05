import { readFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const selectedInputs = {
  RUST_IMAGE: "rustImage", NODE_IMAGE: "nodeImage", PNPM_VERSION: "pnpmVersion",
  CARGO_XWIN_VERSION: "cargoXwinVersion", WINDOWS_TARGET: "windowsTarget", DEBIAN_SNAPSHOT: "debianSnapshot",
  NSIS_VERSION: "nsisVersion", NSIS_SOURCE_URL: "nsisSourceUrl", NSIS_SOURCE_SHA256: "nsisSourceSha256",
  NSIS_WINDOWS_URL: "nsisWindowsUrl", NSIS_WINDOWS_SHA256: "nsisWindowsSha256", NSIS_SOURCE_DATE_EPOCH: "nsisSourceDateEpoch",
};

export function verifyReleaseInputs(root) {
  const inputs = JSON.parse(readFileSync(resolve(root, "build/release-inputs.json"), "utf8"));
  const dockerfile = readFileSync(resolve(root, "Dockerfile"), "utf8");
  if (inputs.schemaVersion !== 1) throw new Error("Unknown release input schema");
  for (const [arg, key] of Object.entries(selectedInputs)) {
    if (!dockerfile.split("\n").includes(`ARG ${arg}=${inputs[key]}`)) throw new Error(`Dockerfile ${arg} does not match reviewed inputs`);
  }
  for (const key of ["rustImage", "nodeImage"]) {
    if (!/@sha256:[a-f0-9]{64}$/.test(inputs[key])) throw new Error(`${key} must use an immutable digest`);
  }
  if (!inputs.rustImage.startsWith(`rust:${inputs.rustVersion}-`) || !inputs.nodeImage.startsWith(`node:${inputs.nodeVersion}-`)) throw new Error("Compiler version and image policy differ");
  if (!/^\d{8}T\d{6}Z$/.test(inputs.debianSnapshot)) throw new Error("Debian snapshot is not fixed");
  if (!/^\d+\.\d+$/.test(inputs.nsisVersion) || inputs.nsisSourceUrl !== `https://deb.debian.org/debian/pool/main/n/nsis/nsis_${inputs.nsisVersion}.orig.tar.gz` || inputs.nsisWindowsUrl !== `https://github.com/tauri-apps/binary-releases/releases/download/nsis-${inputs.nsisVersion}/nsis-${inputs.nsisVersion}.zip`) throw new Error("NSIS compiler and Windows materials must use one reviewed release");
  for (const key of ["nsisSourceSha256", "nsisWindowsSha256"]) if (!/^[a-f0-9]{64}$/.test(inputs[key] ?? "")) throw new Error("NSIS archives require exact SHA-256 digests");
  if (!Number.isSafeInteger(inputs.nsisSourceDateEpoch) || inputs.nsisSourceDateEpoch < 315532800) throw new Error("NSIS compiler requires a fixed build timestamp");
  for (const fragment of ["snapshot.debian.org/archive/debian/", "snapshot.debian.org/archive/debian-security/", "--locked", "zip -X", "BUILD-MATERIALS.json", "SOURCE_DATE_EPOCH", '"${NSIS_SOURCE_SHA256}  /tmp/rice-nsis-source.tar.gz"', '"${NSIS_WINDOWS_SHA256}  /tmp/rice-nsis-windows.zip"', "sha256sum --check -", "NSIS_CONFIG_CONST_DATA_PATH=yes PREFIX=/opt/nsis install-compiler", 'VER_MAJOR="${NSIS_VERSION%%.*}" VER_MINOR="${NSIS_VERSION#*.}" VER_REVISION=0 VER_BUILD=0', 'test "$(makensis -VERSION)" = "v${NSIS_VERSION}"', "makensis -V2 scripts/nsis-toolchain-probe.nsi", "--runtime --nsis"]) {
    if (!dockerfile.includes(fragment)) throw new Error(`Missing release input control: ${fragment}`);
  }
  if (!Array.isArray(inputs.remainingNondeterminism) || inputs.remainingNondeterminism.length === 0) throw new Error("Remaining nondeterminism must be documented");
  return inputs;
}

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const inputs = verifyReleaseInputs(root);
  if (process.argv.includes("--runtime")) {
    for (const [arg, key] of Object.entries(selectedInputs)) {
      const selected = process.env[`RICE_BUILD_${arg}`];
      if (selected !== undefined && selected !== String(inputs[key])) throw new Error(`Build argument ${arg} overrides reviewed inputs`);
    }
    if (process.versions.node !== inputs.nodeVersion) throw new Error("Release Node.js version differs from policy");
    const rust = execFileSync("rustc", ["--version"], { encoding: "utf8" }).trim().split(" ")[1];
    if (rust !== inputs.rustVersion) throw new Error("Release Rust version differs from policy");
  }
  if (process.argv.includes("--nsis")) {
    if (execFileSync("makensis", ["-VERSION"], { encoding: "utf8" }).trim() !== `v${inputs.nsisVersion}` || !existsSync("/opt/nsis/share/nsis/Include/Win/RestartManager.nsh")) throw new Error("Installed NSIS toolchain differs from reviewed release");
  }
  console.log("release build input policy checks passed");
}
