import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const config = JSON.parse(await readFile(resolve(root, "src-tauri/tauri.conf.json"), "utf8"));
const capability = JSON.parse(await readFile(resolve(root, "src-tauri/capabilities/default.json"), "utf8"));
const buildSource = await readFile(resolve(root, "src-tauri/build.rs"), "utf8");
const appSource = await readFile(resolve(root, "src-tauri/src/lib.rs"), "utf8");

function fail(message) {
  throw new Error(`Tauri security check failed: ${message}`);
}

function expectEqual(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} must be ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`);
  }
}

const expectedCsp = {
  "default-src": "'self'",
  "script-src": "'self'",
  "script-src-attr": "'none'",
  "style-src": "'self'",
  "style-src-attr": "'unsafe-inline'",
  "connect-src": "ipc: http://ipc.localhost",
  "img-src": "'self' data:",
  "font-src": "'self'",
  "object-src": "'none'",
  "frame-src": "'none'",
  "worker-src": "'none'",
  "media-src": "'none'",
  "base-uri": "'none'",
  "form-action": "'none'",
};
const expectedDevCsp = {
  "default-src": "'self'",
  "script-src": "'self'",
  "script-src-attr": "'none'",
  "style-src": "'self' 'unsafe-inline'",
  "style-src-attr": "'unsafe-inline'",
  "connect-src": "'self' ipc: http://ipc.localhost ws://localhost:1420",
  "img-src": "'self' data:",
  "font-src": "'self'",
  "object-src": "'none'",
  "frame-src": "'none'",
  "worker-src": "'none'",
  "media-src": "'none'",
  "base-uri": "'none'",
  "form-action": "'none'",
};
const security = config.app?.security;
if (!security || typeof security !== "object") {
  fail("app.security is missing");
}
expectEqual(security.csp, expectedCsp, "production CSP");
expectEqual(security.devCsp, expectedDevCsp, "development CSP override");
expectEqual(security.capabilities, ["default"], "enabled capabilities");
for (const forbiddenDevelopmentSource of ["ws://localhost:1420", "http://localhost:1420"]) {
  if (JSON.stringify(security.csp).includes(forbiddenDevelopmentSource)) {
    fail(`production CSP must not contain development source ${forbiddenDevelopmentSource}`);
  }
}
if (security.csp["style-src"].includes("'unsafe-inline'")) {
  fail("production style-src must not allow Vite's inline style injection");
}
if (security.assetProtocol?.enable === true) {
  fail("asset protocol must remain disabled while launcher icons use validated data URLs");
}

const commandManifest = buildSource.match(/const APP_COMMANDS: &\[&str\] = &\[([\s\S]*?)\n\s*\];/);
if (!commandManifest) {
  fail("build.rs must define the application command manifest");
}
const appCommands = [...commandManifest[1].matchAll(/"([a-z][a-z0-9_]*)"/g)].map((match) => match[1]);
const handlerManifest = appSource.match(/\.invoke_handler\(tauri::generate_handler!\[([\s\S]*?)\n\s*\]\)/);
if (!handlerManifest) {
  fail("src-tauri/src/lib.rs must define the invoke handler manifest");
}
const handledCommands = handlerManifest[1]
  .split(",")
  .map((command) => command.trim())
  .filter(Boolean);
expectEqual(appCommands, handledCommands, "application command ACL manifest");

const expectedPermissions = [
  "core:event:allow-listen",
  "core:event:allow-unlisten",
  "core:window:allow-destroy",
  "core:window:allow-is-maximized",
  "core:window:allow-minimize",
  "core:window:allow-toggle-maximize",
  "core:window:allow-start-dragging",
  "core:window:allow-start-resize-dragging",
  "dialog:allow-open",
  ...appCommands.map((command) => `allow-${command.replaceAll("_", "-")}`),
];
expectEqual(capability.windows, ["main"], "capability windows");
expectEqual(capability.permissions, expectedPermissions, "renderer permissions");
if ("remote" in capability) {
  fail("bundled main-window capability must not allow remote origins");
}

const distFlag = process.argv.indexOf("--dist");
if (distFlag !== -1) {
  const distPath = resolve(root, process.argv[distFlag + 1] ?? "dist");
  const html = await readFile(resolve(distPath, "index.html"), "utf8");
  if (/<(?:base|form|iframe|frame|object|embed)\b/i.test(html)) {
    fail("built index.html contains a blocked navigation or embedded-content element");
  }
  for (const match of html.matchAll(/<(?:script|link|img)\b[^>]*?\b(?:src|href)=["']([^"']+)["']/gi)) {
    const source = match[1];
    if (/^(?:[a-z][a-z\d+.-]*:|\/\/)/i.test(source)) {
      fail(`built index.html references a non-bundled asset: ${source}`);
    }
  }
  for (const match of html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)) {
    if (!/\bsrc=["'][^"']+["']/i.test(match[1]) && match[2].trim() !== "") {
      fail("built index.html contains an inline script");
    }
  }
}

const binaryFlag = process.argv.indexOf("--binary");
if (binaryFlag !== -1) {
  const binaryPath = resolve(root, process.argv[binaryFlag + 1] ?? "src-tauri/target/debug/rice");
  const binary = await readFile(binaryPath);
  for (const marker of ["__app-acl__", "connect-src", "ipc: http://ipc.localhost", "script-src-attr", "allow-app-exit"]) {
    if (!binary.includes(Buffer.from(marker))) {
      fail(`built application does not contain the expected CSP/ACL marker: ${marker}`);
    }
  }
}

const checkedArtifacts = [distFlag !== -1 && "frontend assets", binaryFlag !== -1 && "application binary"]
  .filter(Boolean)
  .join(" and ");
console.log(`Tauri renderer security boundary verified${checkedArtifacts ? ` with ${checkedArtifacts}` : ""}.`);
