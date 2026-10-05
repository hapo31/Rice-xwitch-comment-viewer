import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { writeFileSync } from "node:fs";

// These are real core commands, not invented names whose absence could be
// mistaken for an ACL denial. The release build rejects them BEFORE dispatch.
export const deniedCommands = [
  "plugin:event|emit", "plugin:event|emit_to",
  "plugin:image|from_path", "plugin:image|from_bytes",
  "plugin:menu|new", "plugin:menu|popup",
  "plugin:tray|new", "plugin:tray|set_tooltip",
  "plugin:window|close", "plugin:window|set_title", "plugin:dialog|save",
];
export function isAclDenial(command, error) {
  return deniedCommands.includes(command) && error === `Command ${command} not allowed by ACL`;
}
export function validateDebugTarget(targets, port) {
  const pages = targets.filter(x => x.type === "page" && /^http:\/\/tauri\.localhost(?:\/|$)/.test(x.url));
  assert.equal(pages.length, 1, "Exactly one local bundled Rice page is required");
  const url = new URL(pages[0].webSocketDebuggerUrl);
  assert.equal(url.protocol, "ws:");
  assert.ok(["127.0.0.1", "localhost"].includes(url.hostname), "Debugger must stay loopback-only");
  assert.equal(url.port, String(port));
  assert.match(url.pathname, /^\/devtools\/page\/[\w-]+$/);
  assert.equal(url.search + url.hash + url.username + url.password, "");
  return url.href;
}
export function verifyCapabilityProbe(probe, pid, name) {
  assert.equal(probe?.schemaVersion, 1);
  assert.equal(probe.pid, pid);
  assert.equal(probe.status, "success");
  for (const field of ["minimize", "maximize", "restore", "titlebarDrag", "resizeDrag", "backendEvent", "unlisten", "nativeFileDrop", "nativeMultipleFileDialog", "launcherCleanup"]) assert.equal(probe[field], true, `Missing packaged runtime proof: ${field}`);
  assert.equal(probe.titlebarCloseRequested, name === "installed");
  assert.equal(probe.selectedCount, 2);
  assert.equal(probe.droppedCount, 1);
  assert.deepEqual(probe.deniedCommands, deniedCommands);
  return true;
}
async function connect(url) {
  const socket = new WebSocket(url);
  let next = 0;
  const pending = new Map();
  const rejectAll = error => { for (const p of pending.values()) p.reject(error); pending.clear(); };
  socket.addEventListener("message", event => {
    try {
      assert.ok(event.data.length < 1_048_576);
      const message = JSON.parse(event.data), p = pending.get(message.id);
      if (!p) return;
      pending.delete(message.id); clearTimeout(p.timer);
      if (message.error) p.reject(new Error(JSON.stringify(message.error))); else p.resolve(message.result);
    } catch (error) { rejectAll(error); }
  });
  socket.addEventListener("close", () => rejectAll(new Error("Owned WebView debugger closed")));
  socket.addEventListener("error", () => rejectAll(new Error("Owned WebView debugger failed")));
  await new Promise((accept, reject) => {
    const timer = setTimeout(() => reject(new Error("Debugger open timeout")), 10_000);
    socket.addEventListener("open", () => { clearTimeout(timer); accept(); }, { once: true });
    socket.addEventListener("error", () => { clearTimeout(timer); reject(new Error("Debugger connection failed")); }, { once: true });
  });
  return {
    close: () => socket.close(),
    call: (method, params) => new Promise((accept, reject) => {
      const id = ++next;
      const timer = setTimeout(() => { pending.delete(id); reject(new Error(`CDP timeout: ${method}`)); }, 25_000);
      pending.set(id, { resolve: accept, reject: error => { clearTimeout(timer); reject(error); }, timer });
      socket.send(JSON.stringify({ id, method, params }));
    }),
  };
}
function nativeHelper(pid, fixtureRoot) {
  // Windows PowerShell provides the .NET Framework STA/WinForms/UI Automation
  // APIs already installed on the runner. RemoteSigned is process-local only.
  const child = spawn("powershell.exe", ["-NoProfile", "-STA", "-ExecutionPolicy", "RemoteSigned", "-File", fileURLToPath(new URL("windows-capability-native.ps1", import.meta.url)), "-RicePid", String(pid), "-FixtureRoot", fixtureRoot], { stdio: ["pipe", "pipe", "pipe"] });
  let next = 0, stderr = "";
  const pending = new Map();
  child.stderr.on("data", chunk => stderr = (stderr + chunk).slice(-8192));
  const rejectAll = error => { for (const p of pending.values()) p.reject(error); pending.clear(); };
  createInterface({ input: child.stdout }).on("line", line => {
    try {
      const message = JSON.parse(line), p = pending.get(message.id);
      assert.ok(p, "Unexpected native probe response"); pending.delete(message.id);
      if (message.error) p.reject(new Error(message.error)); else p.resolve(message.value);
    } catch (error) { rejectAll(error); }
  });
  child.on("error", rejectAll);
  child.on("exit", code => rejectAll(new Error(`Native probe exited (${code}): ${stderr}`)));
  return {
    stop: () => { child.stdin.end(); if (child.exitCode === null) child.kill(); },
    call: (action, data = {}) => new Promise((accept, reject) => {
      const id = ++next, timer = setTimeout(() => { pending.delete(id); reject(new Error(`Native probe timeout: ${action}: ${stderr}`)); }, 35_000);
      pending.set(id, { resolve: value => { clearTimeout(timer); accept(value); }, reject: error => { clearTimeout(timer); reject(error); } });
      child.stdin.write(`${JSON.stringify({ id, action, ...data })}\n`);
    }),
  };
}
async function run() {
  assert.equal(process.platform, "win32");
  assert.equal(process.env.GITHUB_ACTIONS, "true");
  assert.equal(process.env.RUNNER_ENVIRONMENT, "github-hosted");
  const [portText, pidText, fixtureRoot, reportFile, name] = process.argv.slice(2);
  const port = Number(portText), pid = Number(pidText);
  assert.ok(Number.isInteger(port) && port >= 1024 && port < 65536);
  assert.ok(Number.isInteger(pid) && pid > 0);
  assert.ok(["portable", "installed"].includes(name));
  const files = ["capability-a.exe", "capability-b.exe", "capability-drop.exe"].map(file => resolve(fixtureRoot, file));
  let target;
  for (let n = 0; n < 100; n++) {
    try { target = validateDebugTarget(await (await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(1000), redirect: "error" })).json(), port); break; }
    catch { await delay(200); }
  }
  assert.ok(target, "The owned packaged WebView did not expose its local debugger");
  const cdp = await connect(target), native = nativeHelper(pid, fixtureRoot);
  const evaluate = async expression => {
    const result = await cdp.call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  const invoke = (command, args = {}) => evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)}, ${JSON.stringify(args)})`);
  const wait = async (predicate, label) => {
    for (let n = 0; n < 100; n++) { if (await predicate()) return; await delay(100); }
    throw new Error(`Packaged UI did not satisfy: ${label}`);
  };
  const click = selector => evaluate(`(() => { const button=document.querySelector(${JSON.stringify(selector)}); if(!button || button.disabled) throw Error('Missing enabled real UI button'); button.click(); return true; })()`);
  const proof = { schemaVersion: 1, pid, status: "running", deniedCommands: [] };
  try {
    await wait(() => evaluate(`!!window.__TAURI_INTERNALS__?.invoke && !!document.querySelector('button[aria-label="最大化"]')`), "production frontend ready");
    assert.equal((await invoke("settings_get")).launcher.items.length, 0, "Never alter pre-existing Launcher items");
    await evaluate(`(async () => {
      window.__riceCapabilityProbe = { events: [], listeners: [] };
      for(const event of ['app://log','tauri://resize','tauri://drag-drop']) {
        const handler=window.__TAURI_INTERNALS__.transformCallback(payload => window.__riceCapabilityProbe.events.push(payload));
        const id=await window.__TAURI_INTERNALS__.invoke('plugin:event|listen',{event,target:{kind:'Any'},handler});
        window.__riceCapabilityProbe.listeners.push({event,id,handler});
      }
      return true;
    })()`);
    for (const command of deniedCommands) {
      const error = await evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)}, { event:'rice-ci-denied', target:{kind:'Any'}, payload:null, path:${JSON.stringify(files[0])}, bytes:[], label:'main', value:'unexpected', options:{} }).then(() => null, error => String(error))`);
      assert.ok(isAclDenial(command, error), `Not an ACL denial for ${command}: ${error}`);
      proof.deniedCommands.push(command);
    }
    await invoke("settings_update", { patch: {} });
    await wait(() => evaluate(`window.__riceCapabilityProbe.events.some(x => x.event==='app://log' && x.payload.message==='設定を保存しました。')`), "real backend settings log event");
    proof.backendEvent = true;
    await native.call("prepare");
    await click('button[aria-label="最大化"]');
    await wait(async () => (await native.call("state")).maximized && await invoke("plugin:window|is_maximized", { label: "main" }), "native maximize"); proof.maximize = true;
    await click('button[aria-label="元に戻す"]');
    await wait(async () => !(await native.call("state")).maximized && await evaluate(`!!document.querySelector('button[aria-label="最大化"]')`), "native restore and resize subscription"); proof.restore = true;
    await click('button[aria-label="最小化"]');
    await wait(async () => (await native.call("state")).minimized, "native minimize"); proof.minimize = true;
    await native.call("restore");
    const geometry = await evaluate(`({width:innerWidth,height:innerHeight,scale:devicePixelRatio})`);
    let before = await native.call("state");
    let after = await native.call("drag", { x: Math.round(250 * geometry.scale), y: Math.round(16 * geometry.scale), dx: 50, dy: 25 });
    assert.ok(Math.abs(after.left - before.left) >= 25 && Math.abs(after.top - before.top) >= 10, "Titlebar native drag must move the real HWND"); proof.titlebarDrag = true;
    before = after;
    await evaluate(`window.__riceCapabilityProbe.events=window.__riceCapabilityProbe.events.filter(x=>x.event!=='tauri://resize'); true`);
    after = await native.call("drag", { x: Math.round((geometry.width - 2) * geometry.scale), y: Math.round(geometry.height / 2 * geometry.scale), dx: -20, dy: 0 });
    assert.ok(Math.abs(after.width - before.width) >= 12, "Resize handle must resize the real HWND");
    await wait(() => evaluate(`window.__riceCapabilityProbe.events.some(x=>x.event==='tauri://resize')`), "real native resize event"); proof.resizeDrag = true;
    await evaluate(`location.hash='/launcher'; true`);
    await wait(() => evaluate(`!!document.querySelector('button[aria-label="アプリをランチャーに追加"]:not(:disabled)')`), "production Launcher ready");
    await click('button[aria-label="アプリをランチャーに追加"]');
    const selected = await native.call("dialog", { paths: files.slice(0, 2) });
    assert.equal(selected.nativeDialog, true); assert.equal(selected.ownerPid, pid); assert.equal(selected.selectedCount, 2);
    await wait(async () => (await invoke("settings_get")).launcher.items.length === 2, "two native dialog-selected executables registered");
    const selectedItems = (await invoke("settings_get")).launcher.items;
    for (const file of files.slice(0, 2)) assert.ok(selectedItems.some(x => x.target.toLowerCase() === file.toLowerCase()));
    proof.nativeMultipleFileDialog = true; proof.selectedCount = 2;
    const dropGeometry = await evaluate(`({x:innerWidth/2,y:innerHeight/2,scale:devicePixelRatio})`);
    assert.equal(await native.call("drop", { x: Math.round(dropGeometry.x * dropGeometry.scale), y: Math.round(dropGeometry.y * dropGeometry.scale), paths: [files[2]] }), "Copy");
    await wait(async () => (await invoke("settings_get")).launcher.items.length === 3, "real OLE file-drop registered by production listener");
    await wait(() => evaluate(`window.__riceCapabilityProbe.events.some(x=>x.event==='tauri://drag-drop' && x.payload.paths.some(p=>p.toLowerCase()===${JSON.stringify(files[2].toLowerCase())}))`), "real file-drop event with exact fixture path");
    proof.nativeFileDrop = true; proof.droppedCount = 1;
    for (const item of (await invoke("settings_get")).launcher.items) await invoke("launcher_remove", { itemId: item.id });
    assert.equal((await invoke("settings_get")).launcher.items.length, 0); proof.launcherCleanup = true;
    await evaluate(`(async () => { for(const {event,id,handler} of window.__riceCapabilityProbe.listeners) {
      window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(event,id);
      await window.__TAURI_INTERNALS__.invoke('plugin:event|unlisten',{event,eventId:id});
      window.__TAURI_INTERNALS__.unregisterCallback(handler);
    } window.__riceCapabilityProbe.events=[]; return true; })()`);
    await invoke("settings_update", { patch: {} }); await delay(300);
    assert.equal(await evaluate(`window.__riceCapabilityProbe.events.length`), 0); proof.unlisten = true;
    proof.titlebarCloseRequested = name === "installed";
    if (proof.titlebarCloseRequested) {
      const closePoint = await evaluate(`({x:innerWidth-22,y:16,scale:devicePixelRatio})`);
      await native.call("click", { x: Math.round(closePoint.x * closePoint.scale), y: Math.round(closePoint.y * closePoint.scale) });
    }
    proof.status = "success";
    verifyCapabilityProbe(proof, pid, name);
    writeFileSync(reportFile, JSON.stringify(proof, null, 2));
    console.log(`Packaged ${name} capability and native UI probes passed (${deniedCommands.length} ACL denials)`);
  } finally { native.stop(); cdp.close(); }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await run();
}
