import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { evaluateAudit, readExceptions, validateExceptions } from "./audit-policy.mjs";

const policy = { schemaVersion: 1, npmBlockingSeverities: ["high", "critical"], cargoBlockVulnerabilities: true, cargoBlockWarnings: true };
const cargo = { database: { "advisory-count": 100 }, vulnerabilities: { list: [] }, warnings: {} };
const npm = { metadata: { vulnerabilities: {} }, advisories: {} };
const exception = { scope: "cargo", id: "RUSTSEC-2024-0429", owner: "hapo31", rationale: "Reviewed Linux-only upstream dependency; no compatible fixed release.", expiresOn: "2026-10-21" };
test("exception is valid through the stated date and rejected the next day", () => {
  assert.equal(validateExceptions({ schemaVersion: 1, exceptions: [exception] }, "2026-10-21").size, 1);
  assert.throws(() => validateExceptions({ schemaVersion: 1, exceptions: [exception] }, "2026-10-22"));
});
test("reject missing review fields, invalid dates, duplicates and unknown scope", () => {
  for (const change of [{ owner: "" }, { rationale: "" }, { expiresOn: "2026-02-30" }, { scope: "all" }]) assert.throws(() => validateExceptions({ schemaVersion: 1, exceptions: [{ ...exception, ...change }] }, "2026-01-01"));
  assert.throws(() => validateExceptions({ schemaVersion: 1, exceptions: [exception, exception] }, "2026-01-01"));
});
test("Cargo vulnerabilities and maintenance warnings block unless explicitly reviewed", () => {
  const report = { ...cargo, warnings: { unsound: [{ advisory: { id: exception.id } }] } };
  assert.equal(evaluateAudit(report, npm, policy, new Map()).blocked.length, 1);
  const reviewed = evaluateAudit(report, npm, policy, validateExceptions({ schemaVersion: 1, exceptions: [exception] }, "2026-10-05"));
  assert.equal(reviewed.blocked.length, 0);
  assert.equal(reviewed.findings.length, 1);
});
test("npm high/critical block while lower severity remains visible", () => {
  for (const severity of ["low", "moderate", "high", "critical"]) {
    const report = { ...npm, advisories: { one: { github_advisory_id: "GHSA-abcd-1234-abcd", severity } } };
    assert.equal(evaluateAudit(cargo, report, policy, new Map()).blocked.length, ["high", "critical"].includes(severity) ? 1 : 0);
  }
});
test("outages, invalid schema and empty database fail closed", () => {
  for (const report of [{}, { ...cargo, database: { "advisory-count": 0 } }]) assert.throws(() => evaluateAudit(report, npm, policy, new Map()));
  assert.throws(() => evaluateAudit(cargo, { error: "registry outage" }, policy, new Map()));
  assert.throws(() => evaluateAudit(cargo, { ...npm, metadata: { vulnerabilities: { high: 1 } } }, policy, new Map()));
});

test("TOML comments and single quotes preserve only the reviewed Cargo ignores", (t) => {
  const root = mkdtempSync(join(tmpdir(), "rice-audit-policy-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, ".cargo"), { recursive: true });
  mkdirSync(join(root, "security"), { recursive: true });
  writeFileSync(join(root, "security/advisory-exceptions.json"), readFileSync(new URL("../security/advisory-exceptions.json", import.meta.url)));
  writeFileSync(join(root, ".cargo/audit.toml"), "# reviewed allowlist\n[advisories]\nignore = ['RUSTSEC-2024-0370', 'RUSTSEC-2024-0429']\n");
  assert.equal(readExceptions(root).size, 2);
  writeFileSync(join(root, ".cargo/audit.toml"), "[advisories]\nignore = ['RUSTSEC-2024-0370', 'RUSTSEC-2024-0429', 'RUSTSEC-2026-9999']\n");
  assert.throws(() => readExceptions(root), /Unreviewed cargo-audit ignore/);
  writeFileSync(join(root, ".cargo/audit.toml"), "[advisories]\nignore = ['RUSTSEC-2024-0370', 'RUSTSEC-2024-0429']\nallow = true\n");
  assert.throws(() => readExceptions(root), /Unknown cargo-audit TOML policy shape/);
});
