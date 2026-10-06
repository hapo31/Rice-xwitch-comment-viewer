import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { readToml, readYaml } from "./config-parsers.mjs";

export function verifyTauriVersions(root, { installed = false } = {}) {
  const pkg = JSON.parse(readFileSync(resolve(root, "package.json"), "utf8"));
  const lock = readToml(resolve(root, "src-tauri/Cargo.lock"));
  if (!Array.isArray(lock.package) || lock.package.some((pkg) => !pkg || typeof pkg.name !== "string" || typeof pkg.version !== "string")) throw new Error("Cargo.lock must contain valid package entries");
  const packages = lock.package;
  const cores = packages.filter((pkg) => pkg?.name === "tauri");
  if (cores.length !== 1) throw new Error("Expected exactly one locked Tauri core");
  const rust = cores[0].version;
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
  const plugins = Object.keys({ ...pkg.dependencies, ...pkg.devDependencies }).filter(name => name.startsWith("@tauri-apps/plugin-"));
  if (plugins.length) {
    const npmLock = readYaml(resolve(root, "pnpm-lock.yaml"));
    if (!npmLock || typeof npmLock !== "object" || !npmLock.dependencies || !npmLock.devDependencies) throw new Error("pnpm lock must contain dependency groups");
    for (const name of plugins) {
      const rustName = `tauri-${name.slice("@tauri-apps/".length)}`;
      const crates = packages.filter((pkg) => pkg?.name === rustName);
      if (crates.length !== 1) throw new Error(`Expected exactly one locked Tauri plugin: ${rustName}`);
      const rustVersion = crates[0].version;
      const lockedPlugin = npmLock.dependencies[name] ?? npmLock.devDependencies[name];
      const jsVersion = typeof lockedPlugin?.version === "string" ? lockedPlugin.version.match(/^(\d+\.\d+\.\d+)(?:\([^\n]*\))?$/)?.[1] : undefined;
      if (!jsVersion) throw new Error(`Missing locked JS Tauri plugin: ${name}`);
      if (minor(jsVersion) !== minor(rustVersion)) throw new Error(`Tauri Rust/JS major-minor mismatch: ${name}`);
      if (installed) {
        const actual = JSON.parse(readFileSync(resolve(root, "node_modules", name, "package.json"), "utf8"));
        if (actual.version !== jsVersion) throw new Error(`Installed Tauri package differs from reviewed input: ${name}`);
      }
    }
  }
  return rust;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  verifyTauriVersions(resolve(fileURLToPath(new URL("..", import.meta.url))), { installed: process.argv.includes("--installed") });
  console.log("Locked Tauri Rust/API/CLI/plugin compatibility verified");
}
