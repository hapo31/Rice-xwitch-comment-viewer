import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { verifyBundle } from "./verify-release-artifacts.mjs";

const fail = message => { throw new Error(message); };
export function verifyWindowsSmoke(report, manifest, manifestHash, runId, jobs) {
  if (report.schemaVersion !== 1 || report.status !== "success" || report.commit !== manifest.commit || report.tag !== manifest.tag || report.runId !== String(runId) || report.artifactManifestSha256 !== manifestHash) fail("Windows smoke receipt source/run/artifact mismatch");
  const requiredProbes = ["portable", "silent-install", "installed", "silent-uninstall"];
  if (!Array.isArray(report.probes) || !isDeepStrictEqual(report.probes.map(x => x.name).sort(), requiredProbes.sort())) fail("Missing/duplicate Windows smoke probes");
  const portableHash = manifest.portableEntries.find(entry => entry.name === "rice.exe")?.sha256;
  const installedHash = manifest.nsisExecutable?.sha256;
  if (!/^[a-f0-9]{64}$/.test(portableHash ?? "") || !/^[a-f0-9]{64}$/.test(installedHash ?? "") || manifest.nsisExecutable?.bundleType !== "nsis" || manifest.nsisExecutable?.name !== "rice.exe") fail("Missing exact portable/NSIS executable expectations");
  for (const probe of report.probes) {
    const exeHash = probe.name === "portable" ? portableHash : installedHash;
    if (probe.exitCode !== 0) fail("Failed Windows smoke probe");
    if (["portable", "installed"].includes(probe.name) && (probe.windowShown !== true || !Number.isFinite(probe.survivedMs) || probe.survivedMs < 5000 || probe.sha256 !== exeHash || !Number.isInteger(probe.pid) || probe.pid < 1)) fail("Unproven application startup/normal exit");
  }
  function requireJob(names, stepNames) {
    const matches = jobs.filter(job => names.includes(job.name));
    if (matches.length !== 1 || matches[0].conclusion !== "success" || matches[0].status !== "completed") fail("Required Windows CI job missing/failed/ambiguous");
    for (const name of stepNames) {
      const steps = matches[0].steps?.filter(step => step.name === name) ?? [];
      if (steps.length !== 1 || steps[0].conclusion !== "success" || steps[0].status !== "completed") fail("Required Windows test step missing/skipped/failed");
    }
  }
  // The receipt alone is not proof: a prior tag lacking the current runtime
  // gate must not gain publication just by supplying a plausible JSON file.
  requireJob(["Smoke exact Windows release artifacts"], ["Install, launch and uninstall exact release candidates"]);
  requireJob(["Windows production tests / windows-focus", "windows-tests / windows-focus"], [
    "Run all Windows targets and features, including native credentials and process adapters",
    "Verify real second launch exits, preserves settings and restores foreground window",
    "Verify maximum Launcher rendering, heap budget and production IPC rejection",
    "Verify future settings survive startup, rejected production IPC and normal exit",
  ]);
  return true;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [directory, reportFile, jobsFile, runId, tag, commit] = process.argv.slice(2);
  if (!directory || !reportFile || !jobsFile || !/^\d+$/.test(runId ?? "")) fail("Usage: artifacts report jobs.ndjson runId tag commit");
  const root = resolve(process.env.RICE_RELEASE_ROOT ?? fileURLToPath(new URL("..", import.meta.url)));
  const { manifest } = verifyBundle(root, directory, { tag: tag || null, commit });
  const manifestHash = createHash("sha256").update(readFileSync(join(directory, "ARTIFACT-MANIFEST.json"))).digest("hex");
  const jobs = readFileSync(jobsFile, "utf8").trim().split("\n").filter(Boolean).map(line => JSON.parse(line));
  verifyWindowsSmoke(JSON.parse(readFileSync(reportFile, "utf8")), manifest, manifestHash, runId, jobs);
  console.log("Exact artifact-bound Windows smoke and required CI jobs verified");
}
