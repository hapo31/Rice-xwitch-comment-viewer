import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { readExceptions, evaluateAudit } from "./audit-policy.mjs";

const root = process.cwd();
const policy = JSON.parse(readFileSync("security/audit-policy.json", "utf8"));
const exceptions = readExceptions(root);
const reportDir = resolve(process.env.RICE_AUDIT_REPORT_DIR ?? "audit-reports");
mkdirSync(reportDir, { recursive: true });
function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 });
  if (result.error || result.signal || ![0, 1].includes(result.status)) throw new Error(`${command} scanner unavailable: ${result.error?.message ?? result.stderr}`);
  writeFileSync(join(reportDir, `${command}-audit.json`), result.stdout);
  let report;
  try { report = JSON.parse(result.stdout); } catch { throw new Error(`${command} returned no valid audit JSON: ${result.stderr}`); }
  return report;
}
const cargoVersion = spawnSync("cargo", ["audit", "--version"], { encoding: "utf8" });
if (cargoVersion.status !== 0 || !cargoVersion.stdout.includes(policy.cargoAuditVersion)) throw new Error("Pinned cargo-audit is required");
const pnpmVersion = spawnSync("pnpm", ["--version"], { encoding: "utf8" });
if (pnpmVersion.status !== 0 || pnpmVersion.stdout.trim() !== policy.pnpmVersion) throw new Error("Pinned pnpm is required");
// Isolate the scanner from repository ignore config: every finding is evaluated
// here against validated, date-bounded exceptions instead of silently hidden.
const scannerDirectory = mkdtempSync(join(tmpdir(), "rice-strict-audit-"));
try {
  const cargo = run("cargo", ["audit", "--json", "--deny", "warnings", "--file", join(root, "src-tauri/Cargo.lock")], scannerDirectory);
  const npm = run("pnpm", ["audit", "--json"]);
  const result = evaluateAudit(cargo, npm, policy, exceptions);
  writeFileSync(join(reportDir, "policy-result.json"), JSON.stringify({ checkedAt: new Date().toISOString(), cargoDatabase: cargo.database, ...result }, null, 2) + "\n");
  for (const finding of result.findings) console.log(`${finding.scope} ${finding.id}: ${finding.exception ? `reviewed exception until ${finding.exception.expiresOn}` : finding.blocking ? "BLOCKED" : "reported below blocking severity"}`);
  if (result.blocked.length) throw new Error(`${result.blocked.length} advisory policy violations; release must stop`);
  console.log("Dependency policy passed; reviewed exceptions are not resolved advisories.");
} finally { rmSync(scannerDirectory, { recursive: true, force: true }); }
