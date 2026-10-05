import { test } from "node:test";
import assert from "node:assert/strict";
import { deniedCommands, isAclDenial, validateDebugTarget, verifyCapabilityProbe } from "./probe-windows-capabilities.mjs";
const target = { type: "page", url: "http://tauri.localhost/#/chat", webSocketDebuggerUrl: "ws://127.0.0.1:9222/devtools/page/owned-page" };
test("accepts only the owned port and bundled local page", () => {
  assert.equal(validateDebugTarget([target], 9222), target.webSocketDebuggerUrl);
});
for (const [name, edit] of [
  ["foreign port", x => x.webSocketDebuggerUrl = "ws://127.0.0.1:9333/devtools/page/owned-page"],
  ["public debugger", x => x.webSocketDebuggerUrl = "ws://192.0.2.1:9222/devtools/page/owned-page"],
  ["remote renderer", x => x.url = "https://example.com/"],
  ["similar hostname", x => x.url = "http://tauri.localhost.evil/"],
  ["redirected endpoint", x => x.webSocketDebuggerUrl = "wss://example.com/devtools/page/owned-page"],
  ["credentials", x => x.webSocketDebuggerUrl = "ws://user@localhost:9222/devtools/page/owned-page"],
]) test(`rejects ${name}`, () => { const changed = structuredClone(target); edit(changed); assert.throws(() => validateDebugTarget([changed], 9222)); });
test("rejects missing or ambiguous pages", () => { assert.throws(() => validateDebugTarget([], 9222)); assert.throws(() => validateDebugTarget([target, target], 9222)); });
test("missing command, argument errors and feature absence are not accepted as ACL denials", () => {
  for (const command of deniedCommands) {
    assert.equal(isAclDenial(command, `Command ${command} not allowed by ACL`), true);
    for (const error of ["Command not found", "Plugin not found", "missing required key", "from_bytes is only supported if image-png is enabled", null]) assert.equal(isAclDenial(command, error), false);
  }
  assert.equal(isAclDenial("plugin:made-up|absent", "Command plugin:made-up|absent not allowed by ACL"), false);
});
function proof() { return { schemaVersion: 1, pid: 123, status: "success", deniedCommands: [...deniedCommands], minimize: true, maximize: true, restore: true, titlebarDrag: true, resizeDrag: true, backendEvent: true, unlisten: true, nativeFileDrop: true, nativeMultipleFileDialog: true, launcherCleanup: true, titlebarCloseRequested: true, selectedCount: 2, droppedCount: 1 }; }
test("accepts the complete installed runtime proof", () => { assert.equal(verifyCapabilityProbe(proof(), 123, "installed"), true); });
for (const field of ["minimize", "maximize", "restore", "titlebarDrag", "resizeDrag", "backendEvent", "unlisten", "nativeFileDrop", "nativeMultipleFileDialog", "launcherCleanup", "titlebarCloseRequested"]) test(`rejects missing real UI proof: ${field}`, () => { const changed=proof(); delete changed[field]; assert.throws(() => verifyCapabilityProbe(changed,123,"installed")); });
