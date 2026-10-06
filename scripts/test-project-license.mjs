import { test } from "node:test";
import assert from "node:assert/strict";
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import { verifyProjectLicense } from "./verify-project-license.mjs";
const source = resolve(fileURLToPath(new URL("..", import.meta.url)));
const files = ["LICENSE", "package.json", "src-tauri/Cargo.toml", "src-tauri/tauri.conf.json", "Dockerfile", "README.md", "CONTRIBUTING.md"];
test("existing MIT license agrees across project and distribution metadata", () => assert.equal(verifyProjectLicense(source), true));
test("Cargo metadata accepts equivalent single-quoted values and reordered keys", () => {
  const directory = mkdtempSync(join(tmpdir(), "rice-project-license-toml-"));
  try {
    for (const file of files) { mkdirSync(dirname(join(directory, file)), { recursive: true }); cpSync(join(source, file), join(directory, file)); }
    const cargoPath = join(directory, "src-tauri/Cargo.toml");
    const cargo = readFileSync(cargoPath, "utf8").replace('license = "MIT"', "license = 'MIT' # same reviewed value").replace('repository = "https://github.com/hapo31/Rice-xwitch-comment-viewer"', "repository = 'https://github.com/hapo31/Rice-xwitch-comment-viewer'");
    writeFileSync(cargoPath, cargo);
    assert.equal(verifyProjectLicense(directory), true);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});
for (const [name, path, change] of [
  ["changed license text", "LICENSE", (text) => text + "different terms"],
  ["different package license", "package.json", (text) => text.replace('"license": "MIT"', '"license": "GPL-3.0"')],
  ["missing Cargo license", "src-tauri/Cargo.toml", (text) => text.replace('license = "MIT"', '')],
  ["missing offline installer text", "src-tauri/tauri.conf.json", (text) => text.replace('"../LICENSE": "LICENSE"', '"../LICENSE": "other"')],
  ["missing portable text", "Dockerfile", (text) => text.replaceAll('rice.exe LICENSE', 'rice.exe')],
  ["missing contribution conditions", "CONTRIBUTING.md", (text) => text.replace('inbound=outbound', '')],
]) {
  test(`reject ${name}`, () => {
    const directory = mkdtempSync(join(tmpdir(), "rice-project-license-"));
    try {
      for (const file of files) { mkdirSync(dirname(join(directory, file)), { recursive: true }); cpSync(join(source, file), join(directory, file)); }
      writeFileSync(join(directory, path), change(readFileSync(join(directory, path), "utf8")));
      assert.throws(() => verifyProjectLicense(directory));
    } finally { rmSync(directory, { recursive: true, force: true }); }
  });
}
