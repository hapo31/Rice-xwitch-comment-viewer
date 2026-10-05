import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export function verifyTauriVersions(root, { installed = false } = {}) {
  const pkg = JSON.parse(readFileSync(resolve(root, "package.json"), "utf8"));
  const lock = readFileSync(resolve(root, "src-tauri/Cargo.lock"), "utf8");
  const cores = lock.split("[[package]]").filter(block => /^name = "tauri"$/m.test(block));
  if (cores.length !== 1) throw new Error("Expected exactly one locked Tauri core");
  const rust = cores[0].match(/^version = "([^"]+)"$/m)?.[1];
  const minor = version => {
    if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) throw new Error("Tauri framework inputs must use exact reviewed versions");
    return version.split(".").slice(0, 2).join(".");
  };
  for (const [name, version] of [["@tauri-apps/api", pkg.dependencies?.["@tauri-apps/api"]], ["@tauri-apps/cli", pkg.devDependencies?.["@tauri-apps/cli"]]]) {
    if (minor(version) !== minor(rust)) throw new Error(`Tauri Rust/JS major-minor mismatch: ${name}`);
    if (installed) {
      const actual = JSON.parse(readFileSync(resolve(root, "node_modules", name, "package.json"), "utf8"));
      if (actual.version !== version) throw new Error(`Installed Tauri package differs from reviewed input: ${name}`);
    }
  }
  return rust;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  verifyTauriVersions(resolve(fileURLToPath(new URL("..", import.meta.url))), { installed: process.argv.includes("--installed") });
  console.log("Locked Tauri Rust/API/CLI compatibility verified");
}
