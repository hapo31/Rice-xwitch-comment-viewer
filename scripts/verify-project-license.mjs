import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { readToml } from "./config-parsers.mjs";

// Existing owner-selected MIT text, not a new license decision.
const approvedLicenseHash = "eeb4b00cfe4a9c135ab47b643c44f4c0b747318c0d52cee8580bf7c3d2ca0667";
export function verifyProjectLicense(root, { bundle = false } = {}) {
  const read = (path) => readFileSync(resolve(root, path), "utf8");
  const license = read("LICENSE");
  if (createHash("sha256").update(license).digest("hex") !== approvedLicenseHash) throw new Error("Rice LICENSE is missing or differs from the reviewed MIT text; explicit owner review is required");
  const npm = JSON.parse(read("package.json"));
  const cargo = readToml(resolve(root, "src-tauri/Cargo.toml"));
  const config = JSON.parse(read("src-tauri/tauri.conf.json"));
  if (npm.license !== "MIT" || npm.author !== "Rice contributors" || cargo.package?.license !== "MIT" || JSON.stringify(cargo.package?.authors) !== JSON.stringify(["Rice contributors"])) throw new Error("npm/Cargo license and author metadata must agree with LICENSE");
  const repository = "https://github.com/hapo31/Rice-xwitch-comment-viewer";
  if (npm.repository !== repository || cargo.package?.repository !== repository) throw new Error("Project repository metadata differs");
  if (config.bundle.license !== "MIT" || config.bundle.licenseFile !== "../LICENSE" || config.bundle.resources?.["../LICENSE"] !== "LICENSE" || config.bundle.copyright !== "Copyright (c) 2026 Rice contributors") throw new Error("NSIS license presentation and offline installed LICENSE must be configured");
  const docker = read("Dockerfile");
  if (!docker.includes("COPY LICENSE ./LICENSE") || !docker.includes("cp LICENSE /out/LICENSE") || !/zip -X -9 [^\n]*rice\.exe LICENSE/.test(docker)) throw new Error("Installer/release and portable LICENSE packaging paths must be retained");
  if (!bundle) {
    if (!read("README.md").includes("[MIT License](./LICENSE)")) throw new Error("README must reference canonical project license");
    if (!read("CONTRIBUTING.md").includes("inbound=outbound")) throw new Error("Inbound contribution permission must be explicit");
  }
  return true;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  verifyProjectLicense(resolve(fileURLToPath(new URL("..", import.meta.url))), { bundle: process.argv.includes("--bundle") });
  console.log("Rice MIT license consistency and packaging policy passed");
}
