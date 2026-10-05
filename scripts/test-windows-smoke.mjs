import { test } from "node:test";
import assert from "node:assert/strict";
import { verifyWindowsSmoke } from "./verify-windows-smoke.mjs";
import { deniedCommands } from "./probe-windows-capabilities.mjs";
function fixture() {
  const exeHash = "e".repeat(64), installedHash = "d".repeat(64), manifestHash = "f".repeat(64);
  const manifest = { commit: "a".repeat(40), tag: "v0.2.3", portableEntries: [{ name: "rice.exe", sha256: exeHash }], nsisExecutable: { name: "rice.exe", sha256: installedHash, bundleType: "nsis" } };
  const report = { schemaVersion: 1, status: "success", commit: manifest.commit, tag: manifest.tag, runId: "123", artifactManifestSha256: manifestHash, probes: [
    { name: "portable", pid: 1, windowShown: true, survivedMs: 5000, exitCode: 0, sha256: exeHash },
    { name: "silent-install", exitCode: 0 },
    { name: "installed", pid: 2, windowShown: true, survivedMs: 5000, exitCode: 0, sha256: installedHash },
    { name: "silent-uninstall", exitCode: 0 },
  ] };
  for (const probe of report.probes.filter(x => ["portable", "installed"].includes(x.name))) {
    probe.capabilities = { schemaVersion: 1, status: "success", pid: probe.pid, deniedCommands, minimize: true, maximize: true, restore: true, titlebarDrag: true, resizeDrag: true, backendEvent: true, unlisten: true, nativeFileDrop: true, nativeMultipleFileDialog: true, launcherCleanup: true, titlebarCloseRequested: probe.name === "installed", nativeCloseRequested: probe.name === "portable", selectedCount: 2, droppedCount: 1 };
  }
  const job = (name, steps) => ({ name, status: "completed", conclusion: "success", steps: steps.map(name => ({ name, status: "completed", conclusion: "success" })) });
  const jobs = [job("Smoke exact Windows release artifacts", ["Install, launch and uninstall exact release candidates"]), job("Windows production tests / windows-focus", [
    "Validate packaged capability probes and native UI helper",
    "Run all Windows targets and features, including native credentials and process adapters",
    "Verify real second launch exits, preserves settings and restores foreground window",
    "Verify maximum Launcher rendering, heap budget and production IPC rejection",
    "Verify future settings survive startup, rejected production IPC and normal exit",
  ])];
  return { report, manifest, manifestHash, jobs };
}
test("accepts only the source/run/hash-bound receipt with actual required successful jobs", () => {
  const f = fixture(); assert.equal(verifyWindowsSmoke(f.report, f.manifest, f.manifestHash, "123", f.jobs), true);
});
for (const [name, mutate] of [
  ["wrong source", f => f.report.commit = "b".repeat(40)],
  ["wrong tag", f => f.report.tag = "v0.2.2"],
  ["other run", f => f.report.runId = "124"],
  ["other artifact bytes", f => f.report.artifactManifestSha256 = "0".repeat(64)],
  ["faked success marker", f => f.report.status = "running"],
  ["missing uninstall", f => f.report.probes.pop()],
  ["duplicate probes", f => f.report.probes.push(f.report.probes[0])],
  ["early startup exit", f => f.report.probes[0].survivedMs = 4999],
  ["loader/panic exit", f => f.report.probes[2].exitCode = -1073741511],
  ["no native window", f => f.report.probes[0].windowShown = false],
  ["installed different binary", f => f.report.probes[2].sha256 = "0".repeat(64)],
  ["unpatched portable mistaken for installed NSIS", f => f.report.probes[2].sha256 = f.report.probes[0].sha256],
  ["missing NSIS expectation", f => delete f.manifest.nsisExecutable],
  ["wrong NSIS bundle type", f => f.manifest.nsisExecutable.bundleType = "msi"],
  ["missing packaged capability proof", f => delete f.report.probes[0].capabilities],
  ["native drop not checked", f => f.report.probes[0].capabilities.nativeFileDrop = false],
  ["only one dialog selection", f => f.report.probes[0].capabilities.selectedCount = 1],
  ["other process UI proof", f => f.report.probes[2].capabilities.pid = 999],
  ["missing denied command", f => f.report.probes[0].capabilities.deniedCommands = deniedCommands.slice(1)],
  ["titlebar close skipped", f => f.report.probes[2].capabilities.titlebarCloseRequested = false],
  ["native close skipped", f => f.report.probes[0].capabilities.nativeCloseRequested = false],
  ["missing runtime jobs despite plausible receipt", f => f.jobs.length = 0],
  ["failed Windows test", f => f.jobs[1].conclusion = "failure"],
  ["skipped smoke step", f => f.jobs[0].steps[0].conclusion = "skipped"],
  ["no all-targets test", f => f.jobs[1].steps.shift()],
  ["missing explicit headful test", f => f.jobs[1].steps.pop()],
  ["ambiguous retry results", f => f.jobs.push(f.jobs[0])],
]) test(`rejects ${name}`, () => { const f = fixture(); mutate(f); assert.throws(() => verifyWindowsSmoke(f.report, f.manifest, f.manifestHash, "123", f.jobs)); });
