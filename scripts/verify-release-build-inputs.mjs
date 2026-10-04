import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

export function verifyReleaseInputs(root) {
  const inputs = JSON.parse(readFileSync(resolve(root, "build/release-inputs.json"), "utf8"));
  const dockerfile = readFileSync(resolve(root, "Dockerfile"), "utf8");
  if (inputs.schemaVersion !== 1) throw new Error("Unknown release input schema");
  for (const [arg, key] of Object.entries({ RUST_IMAGE: "rustImage", NODE_IMAGE: "nodeImage", PNPM_VERSION: "pnpmVersion", CARGO_XWIN_VERSION: "cargoXwinVersion", WINDOWS_TARGET: "windowsTarget", DEBIAN_SNAPSHOT: "debianSnapshot" })) {
    if (!dockerfile.split("\n").includes(`ARG ${arg}=${inputs[key]}`)) throw new Error(`Dockerfile ${arg} does not match reviewed inputs`);
  }
  for (const key of ["rustImage", "nodeImage"]) {
    if (!/@sha256:[a-f0-9]{64}$/.test(inputs[key])) throw new Error(`${key} must use an immutable digest`);
  }
  if (!inputs.rustImage.startsWith(`rust:${inputs.rustVersion}-`) || !inputs.nodeImage.startsWith(`node:${inputs.nodeVersion}-`)) throw new Error("Compiler version and image policy differ");
  if (!/^\d{8}T\d{6}Z$/.test(inputs.debianSnapshot)) throw new Error("Debian snapshot is not fixed");
  for (const fragment of ["snapshot.debian.org/archive/debian/", "snapshot.debian.org/archive/debian-security/", "--locked", "zip -X", "BUILD-MATERIALS.json", "SOURCE_DATE_EPOCH"]) {
    if (!dockerfile.includes(fragment)) throw new Error(`Missing release input control: ${fragment}`);
  }
  if (!Array.isArray(inputs.remainingNondeterminism) || inputs.remainingNondeterminism.length === 0) throw new Error("Remaining nondeterminism must be documented");
  return inputs;
}

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const inputs = verifyReleaseInputs(root);
  if (process.argv.includes("--runtime")) {
    for (const [arg, key] of Object.entries({ RUST_IMAGE: "rustImage", NODE_IMAGE: "nodeImage", DEBIAN_SNAPSHOT: "debianSnapshot", PNPM_VERSION: "pnpmVersion", CARGO_XWIN_VERSION: "cargoXwinVersion", WINDOWS_TARGET: "windowsTarget" })) {
      const selected = process.env[`RICE_BUILD_${arg}`];
      if (selected !== undefined && selected !== inputs[key]) throw new Error(`Build argument ${arg} overrides reviewed inputs`);
    }
    if (process.versions.node !== inputs.nodeVersion) throw new Error("Release Node.js version differs from policy");
    const rust = execFileSync("rustc", ["--version"], { encoding: "utf8" }).trim().split(" ")[1];
    if (rust !== inputs.rustVersion) throw new Error("Release Rust version differs from policy");
  }
  console.log("release build input policy checks passed");
}
