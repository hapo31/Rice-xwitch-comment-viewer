import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export function validateExceptions(document, today = new Date().toISOString().slice(0, 10)) {
  if (document.schemaVersion !== 1 || !Array.isArray(document.exceptions)) throw new Error("Unknown advisory exception schema");
  const exceptions = new Map();
  for (const entry of document.exceptions) {
    const pattern = entry.scope === "cargo" ? /^RUSTSEC-\d{4}-\d{4}$/ : entry.scope === "npm" ? /^GHSA-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{4}$/ : null;
    if (!pattern?.test(entry.id) || typeof entry.owner !== "string" || !entry.owner.trim() || typeof entry.rationale !== "string" || entry.rationale.trim().length < 20) throw new Error("Advisory exceptions require scope, ID, owner and a reviewed rationale");
    if (!/^\d{4}-\d{2}-\d{2}$/.test(entry.expiresOn ?? "") || Number.isNaN(Date.parse(entry.expiresOn)) || new Date(entry.expiresOn).toISOString().slice(0, 10) !== entry.expiresOn || entry.expiresOn < today) throw new Error(`Expired or invalid exception: ${entry.id}`);
    const key = `${entry.scope}:${entry.id}`;
    if (exceptions.has(key)) throw new Error(`Duplicate exception: ${key}`);
    exceptions.set(key, entry);
  }
  return exceptions;
}

export function evaluateAudit(cargo, npm, policy, exceptions) {
  if (policy.schemaVersion !== 1 || !Array.isArray(policy.npmBlockingSeverities) || !["high", "critical"].every((severity) => policy.npmBlockingSeverities.includes(severity)) || !policy.cargoBlockVulnerabilities || !policy.cargoBlockWarnings) throw new Error("Unknown or incomplete advisory policy");
  if (!cargo.database || !Number.isInteger(cargo.database["advisory-count"]) || cargo.database["advisory-count"] <= 0 || !Array.isArray(cargo.vulnerabilities?.list) || !cargo.warnings || typeof cargo.warnings !== "object") throw new Error("Cargo scanner did not return a valid database-backed report");
  if (!npm.metadata?.vulnerabilities || !npm.advisories || typeof npm.advisories !== "object" || npm.error) throw new Error("npm scanner failed or returned an invalid report");
  const findings = [];
  for (const finding of cargo.vulnerabilities.list) findings.push({ scope: "cargo", id: finding.advisory?.id, blocking: true });
  for (const [kind, entries] of Object.entries(cargo.warnings)) {
    if (!Array.isArray(entries)) throw new Error("Invalid Cargo warning report");
    for (const finding of entries) findings.push({ scope: "cargo", id: finding.advisory?.id ?? `${kind}:${finding.package?.name}@${finding.package?.version}`, blocking: true });
  }
  for (const finding of Object.values(npm.advisories)) {
    if (!finding.github_advisory_id || !["info", "low", "moderate", "high", "critical"].includes(finding.severity)) throw new Error("npm advisory identity or severity is missing");
    findings.push({ scope: "npm", id: finding.github_advisory_id, severity: finding.severity, blocking: policy.npmBlockingSeverities.includes(finding.severity) });
  }
  for (const severity of ["low", "moderate", "high", "critical"]) {
    if ((npm.metadata.vulnerabilities[severity] ?? 0) > 0 && !findings.some((finding) => finding.scope === "npm" && finding.severity === severity)) throw new Error("npm advisory details disagree with the summary");
  }
  const reviewed = findings.map((finding) => ({ ...finding, exception: exceptions.get(`${finding.scope}:${finding.id}`) }));
  return { findings: reviewed, blocked: reviewed.filter((finding) => finding.blocking && !finding.exception) };
}

export function readExceptions(root) {
  const path = resolve(root, "security/advisory-exceptions.json");
  const document = existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : { schemaVersion: 1, exceptions: [] };
  const exceptions = validateExceptions(document);
  const config = resolve(root, ".cargo/audit.toml");
  if (existsSync(config)) {
    const ignored = [...readFileSync(config, "utf8").matchAll(/"(RUSTSEC-\d{4}-\d{4})"/g)].map((match) => match[1]);
    for (const id of ignored) if (!exceptions.has(`cargo:${id}`)) throw new Error(`Unreviewed cargo-audit ignore: ${id}`);
    const expected = [...exceptions.values()].filter((entry) => entry.scope === "cargo").map((entry) => entry.id);
    if (new Set(ignored).size !== ignored.length || ignored.length !== expected.length || expected.some((id) => !ignored.includes(id))) throw new Error("cargo-audit ignore list and reviewed exceptions differ");
  }
  return exceptions;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
  const exceptions = readExceptions(root);
  console.log(`validated ${exceptions.size} time-limited advisory exceptions`);
}
