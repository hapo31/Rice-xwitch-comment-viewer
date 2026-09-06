#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { pathToFileURL } from "node:url";

const requiredChecks = [
  "Frontend unit tests",
  "Rust unit tests",
  "Frontend typecheck",
  "Rust format and clippy",
  "Dev build",
];

function requirePolicy(condition, message) {
  if (!condition) throw new Error(`リリース保護設定が不足しています: ${message}`);
}

// Deliberately accept only the explicit configuration documented in releasing.md.
// Do not guess whether arbitrary ref patterns or inherited bypasses are equivalent.
export function verifyRepositoryPolicy({ repository, rulesets, environment, deployments }) {
  requirePolicy(repository.default_branch === "main", "default branch を main にしてください。");
  const exactRulesets = (target, ref) => rulesets.filter((ruleset) =>
    ruleset.target === target && ruleset.enforcement === "active" &&
    ruleset.conditions?.ref_name?.include?.includes(ref) &&
    ruleset.conditions.ref_name.exclude?.length === 0);
  const tagRulesets = exactRulesets("tag", "refs/tags/v*");
  // GitHub hides bypass_actors from credentials without ruleset write access.
  // Never grant this read-only gate administration/write just to reveal them.
  // Hidden bypass lists must be audited by the administrator (see releasing.md).
  const noVisibleBypass = (ruleset) => !Object.hasOwn(ruleset, "bypass_actors") ||
    (Array.isArray(ruleset.bypass_actors) && ruleset.bypass_actors.length === 0);
  const hasRule = (ruleset, type) => ruleset.rules?.some((rule) => rule.type === type);
  requirePolicy(tagRulesets.some((ruleset) => noVisibleBypass(ruleset) &&
    hasRule(ruleset, "update") && hasRule(ruleset, "deletion") && !hasRule(ruleset, "creation")),
  "v* の update/delete を bypass なしの独立した active tag ruleset で禁止してください。");
  requirePolicy(tagRulesets.some((ruleset) => hasRule(ruleset, "creation") &&
    !hasRule(ruleset, "update") && !hasRule(ruleset, "deletion") &&
    (!Object.hasOwn(ruleset, "bypass_actors") || (ruleset.bypass_actors?.length > 0 &&
      ruleset.bypass_actors.every((actor) =>
        ["Team", "Integration"].includes(actor.actor_type) && actor.bypass_mode === "always")))),
  "v* creation の bypass を指名した release Team / GitHub App のみに限定してください。");

  const mainRules = exactRulesets("branch", "refs/heads/main")
    .filter(noVisibleBypass).flatMap((ruleset) => ruleset.rules ?? []);
  requirePolicy(mainRules.some((rule) => rule.type === "pull_request" &&
    rule.parameters?.required_approving_review_count >= 1), "main の必須 PR review を設定してください。");
  requirePolicy(["deletion", "non_fast_forward"].every((type) => mainRules.some((rule) => rule.type === type)),
    "main の削除と force push を bypass なしで禁止してください。");
  const contexts = mainRules.filter((rule) => rule.type === "required_status_checks")
    .flatMap((rule) => rule.parameters?.required_status_checks ?? []).map((check) => check.context);
  requirePolicy(requiredChecks.every((check) => contexts.includes(check)), "main の必須 CI checks を設定してください。");

  requirePolicy(environment.protection_rules?.some((rule) => rule.type === "required_reviewers" &&
    rule.prevent_self_review === true && rule.reviewers?.length > 0),
  "release environment に required reviewer と prevent self-review を設定してください。");
  requirePolicy(environment.deployment_branch_policy?.custom_branch_policies === true &&
    environment.deployment_branch_policy.protected_branches === false &&
    deployments.branch_policies?.length === 1 && deployments.branch_policies[0].name === "main" &&
    deployments.branch_policies[0].type === "branch",
  "workflow_run の release environment は branch main だけに許可してください。");
}

export function fetchRepositoryPolicy(repositoryName, execute = execFileSync) {
  requirePolicy(/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repositoryName), "repository は owner/name 形式で指定してください。");
  const api = (path) => JSON.parse(execute("gh", ["api", `repos/${repositoryName}${path ? `/${path}` : ""}`], { encoding: "utf8" }));
  const summaries = JSON.parse(execute("gh", ["api", "--paginate", "--slurp",
    `repos/${repositoryName}/rulesets?includes_parents=true&per_page=100`], { encoding: "utf8" })).flat();
  return {
    repository: api(""),
    rulesets: summaries.map(({ id }) => {
      requirePolicy(Number.isSafeInteger(id) && id > 0, "ruleset ID が不正です。");
      return api(`rulesets/${id}`);
    }),
    environment: api("environments/release"),
    deployments: api("environments/release/deployment-branch-policies?per_page=100"),
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    verifyRepositoryPolicy(fetchRepositoryPolicy(process.argv[2] ?? ""));
    console.log("release repository visible policy checks passed");
    console.warn("注意: API が非公開にする ruleset bypass と environment 管理者 bypass、担当者の独立性は管理者による確認が必要です。docs/releasing.md を参照してください。");
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
