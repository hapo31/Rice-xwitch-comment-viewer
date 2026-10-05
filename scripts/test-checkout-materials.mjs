import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const paths = ["LICENSE", "pnpm-lock.yaml", "src-tauri/Cargo.lock", "src-tauri/Cargo.toml"];
function checkout(t, attributes) {
  const scratch = mkdtempSync(join(tmpdir(), "rice-checkout-materials-"));
  t.after(() => rmSync(scratch, { recursive: true, force: true }));
  const source = join(scratch, "source"), target = join(scratch, "checkout"), hooks = join(scratch, "no-hooks");
  mkdirSync(source); mkdirSync(hooks);
  const env = { ...process.env, GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: join(scratch, "unused-global-config") };
  const git = (args) => execFileSync("git", ["-c", `core.hooksPath=${hooks}`, ...args], { env, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  git(["init", "--quiet", source]);
  for (const path of paths) {
    mkdirSync(dirname(join(source, path)), { recursive: true });
    const bytes = readFileSync(join(root, path));
    assert.ok(!bytes.includes(Buffer.from("\r\n")), `${path} must have canonical LF bytes`);
    writeFileSync(join(source, path), bytes);
  }
  if (attributes) writeFileSync(join(source, ".gitattributes"), readFileSync(join(root, ".gitattributes")));
  git(["-C", source, "add", "."]);
  git(["-C", source, "-c", "user.name=Rice test", "-c", "user.email=rice-test@example.invalid", "commit", "--quiet", "-m", "checkout fixture"]);
  git(["clone", "--quiet", "--no-local", "--config", "core.autocrlf=true", source, target]);
  assert.equal(git(["-C", target, "config", "--get", "core.autocrlf"]).trim(), "true");
  return target;
}
test("Windows-style checkout preserves exact reviewed license and lockfile bytes", t => {
  const target = checkout(t, true);
  for (const path of paths) assert.deepEqual(readFileSync(join(target, path)), readFileSync(join(root, path)), path);
});
test("negative control reproduces material digest drift without the explicit attributes", t => {
  const target = checkout(t, false);
  for (const path of paths) {
    const bytes = readFileSync(join(target, path));
    assert.ok(bytes.includes(Buffer.from("\r\n")), `${path} was not converted in the negative control`);
    assert.notDeepEqual(bytes, readFileSync(join(root, path)), path);
  }
});
