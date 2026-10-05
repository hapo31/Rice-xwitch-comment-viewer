import { test } from "node:test";
import assert from "node:assert/strict";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";

const root = new URL("../", import.meta.url);
const reviewedPermissions = [
  "core:event:allow-listen", "core:event:allow-unlisten",
  "core:window:allow-destroy", "core:window:allow-is-maximized",
  "core:window:allow-minimize", "core:window:allow-toggle-maximize",
  "core:window:allow-start-dragging", "core:window:allow-start-resize-dragging",
  "dialog:allow-open",
  "allow-app-exit", "allow-app-open-external-url", "allow-app-build-info", "allow-app-events-snapshot",
  "allow-launcher-add", "allow-launcher-remove", "allow-launcher-launch", "allow-launcher-launch-all",
  "allow-settings-get", "allow-settings-take-recovery-notice", "allow-settings-update",
  "allow-speech-health-check", "allow-speech-health-probe", "allow-speech-connection-diagnostics", "allow-speech-authorize-endpoint",
  "allow-speech-test", "allow-speech-pause", "allow-speech-resume", "allow-speech-skip", "allow-speech-clear",
  "allow-speech-queue-reload", "allow-speech-queue-remove", "allow-speech-queue-dismiss", "allow-speech-queue-dismiss-history", "allow-speech-queue-retry",
  "allow-twitch-start-auth", "allow-twitch-poll-auth", "allow-twitch-validate-auth", "allow-twitch-connect", "allow-twitch-stop-chat", "allow-twitch-get-stored-auth", "allow-twitch-disconnect",
];
const capability = JSON.parse(readFileSync(new URL("src-tauri/capabilities/default.json", root)));
test("the complete core/plugin/custom allowlist matches the reviewed frontend API snapshot", () => {
  assert.deepEqual(capability.permissions, reviewedPermissions);
  assert.deepEqual(capability.windows, ["main"]);
  assert.equal(capability.remote, undefined);
});
function run(mutate) {
  const scratch = mkdtempSync(join(tmpdir(), "rice-capability-"));
  try {
    for (const file of ["scripts/verify-tauri-security.mjs", "src-tauri/tauri.conf.json", "src-tauri/capabilities/default.json", "src-tauri/build.rs", "src-tauri/src/lib.rs"]) {
      mkdirSync(dirname(join(scratch, file)), { recursive: true });
      cpSync(new URL(file, root), join(scratch, file));
    }
    const changed = structuredClone(capability);
    mutate(changed);
    writeFileSync(join(scratch, "src-tauri/capabilities/default.json"), JSON.stringify(changed));
    return spawnSync(process.execPath, [join(scratch, "scripts/verify-tauri-security.mjs")], { encoding: "utf8", timeout: 10_000 });
  } finally { rmSync(scratch, { recursive: true, force: true }); }
}
test("the real policy accepts the exact reviewed capability", () => {
  const result = run(() => {});
  assert.equal(result.status, 0, result.stderr);
});
for (const permission of [
  "core:default", "core:event:default", "core:window:default", "dialog:default",
  "core:event:allow-emit", "core:event:allow-emit-to", "core:image:allow-from-path",
  "core:image:allow-from-bytes", "core:menu:default", "core:tray:default",
  "core:window:allow-close", "core:window:allow-set-title", "dialog:allow-save",
]) test(`the real policy rejects permission expansion: ${permission}`, () => {
  const result = run(x => x.permissions.push(permission));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /renderer permissions/);
});
for (const [name, mutate, message] of [
  ["remote origins", x => x.remote = { urls: ["https://example.com"] }, /capability fields/],
  ["additional webview scope", x => x.webviews = ["*"], /capability fields/],
  ["platform-specific scope", x => x.platforms = ["windows"], /capability fields/],
  ["other capability identifier", x => x.identifier = "other", /capability identifier/],
  ["wildcard windows", x => x.windows = ["*"], /capability windows/],
  ["missing SDK native-close permission", x => x.permissions = x.permissions.filter(p => p !== "core:window:allow-destroy"), /renderer permissions/],
  ["duplicate permissions", x => x.permissions.push(x.permissions[0]), /renderer permissions/],
]) test(`the real policy rejects ${name}`, () => {
  const result = run(mutate); assert.notEqual(result.status, 0); assert.match(result.stderr, message);
});
