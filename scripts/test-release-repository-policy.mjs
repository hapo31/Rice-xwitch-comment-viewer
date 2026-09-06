import assert from "node:assert/strict";
import { test } from "node:test";
import { fetchRepositoryPolicy, verifyRepositoryPolicy } from "./verify-release-repository-policy.mjs";

function fixture() {
  const ruleset = (target, ref, rules, bypass_actors = []) => ({ target, enforcement: "active",
    conditions: { ref_name: { include: [ref], exclude: [] } }, rules, bypass_actors });
  return {
    repository: { default_branch: "main" },
    rulesets: [
      ruleset("tag", "refs/tags/v*", [{ type: "creation" }], [{ actor_type: "Integration", actor_id: 123, bypass_mode: "always" }]),
      ruleset("tag", "refs/tags/v*", [{ type: "update" }, { type: "deletion" }]),
      ruleset("branch", "refs/heads/main", [
        { type: "pull_request", parameters: { required_approving_review_count: 1 } },
        { type: "deletion" }, { type: "non_fast_forward" },
        { type: "required_status_checks", parameters: { required_status_checks:
          ["Frontend unit tests", "Rust unit tests", "Frontend typecheck", "Rust format and clippy", "Dev build"]
            .map((context) => ({ context })) } },
      ]),
    ],
    environment: { protection_rules: [{ type: "required_reviewers", prevent_self_review: true,
      reviewers: [{ type: "User", reviewer: { id: 456 } }] }],
    deployment_branch_policy: { custom_branch_policies: true, protected_branches: false } },
    deployments: { branch_policies: [{ name: "main", type: "branch" }] },
  };
}

test("explicit independent protections allow publication", () => verifyRepositoryPolicy(fixture()));

test("read-only API omits bypass identities; their audit remains an administrator requirement", () => {
  const policy = fixture();
  for (const ruleset of policy.rulesets) delete ruleset.bypass_actors;
  verifyRepositoryPolicy(policy);
});

test("fetch all ruleset pages and exact repository/environment endpoints", () => {
  const policy = fixture();
  const prefix = "repos/owner/repository";
  const responses = new Map([
    [`${prefix}/rulesets?includes_parents=true&per_page=100`, [[{ id: 1 }], [{ id: 2 }, { id: 3 }]]],
    [prefix, policy.repository],
    ...policy.rulesets.map((rule, index) => [`${prefix}/rulesets/${index + 1}`, rule]),
    [`${prefix}/environments/release`, policy.environment],
    [`${prefix}/environments/release/deployment-branch-policies?per_page=100`, policy.deployments],
  ]);
  verifyRepositoryPolicy(fetchRepositoryPolicy("owner/repository", (command, args) => {
    assert.equal(command, "gh");
    if (args.at(-1).includes("?includes_parents")) assert.deepEqual(args.slice(0, -1), ["api", "--paginate", "--slurp"]);
    assert.ok(responses.has(args.at(-1)), `unexpected API endpoint: ${args.at(-1)}`);
    return JSON.stringify(responses.get(args.at(-1)));
  }));
});

test("API failure cannot authorize publication", () => {
  assert.throws(() => fetchRepositoryPolicy("owner/repository", () => { throw new Error("HTTP 403"); }), /403/);
});

const invalidPolicies = {
  "missing settings": (p) => { p.rulesets = []; },
  "disabled ruleset": (p) => { p.rulesets[1].enforcement = "disabled"; },
  "tag update bypass": (p) => { p.rulesets[1].bypass_actors = p.rulesets[0].bypass_actors; },
  "combined creation and immutability": (p) => {
    p.rulesets[1].rules.push({ type: "creation" });
    p.rulesets.shift();
    delete p.rulesets[0].bypass_actors;
  },
  "tag exclusion": (p) => { p.rulesets[1].conditions.ref_name.exclude = ["refs/tags/v1*"]; },
  "broad creation bypass": (p) => { p.rulesets[0].bypass_actors[0].actor_type = "RepositoryRole"; },
  "unprotected main": (p) => { p.rulesets.pop(); },
  "main bypass": (p) => { p.rulesets[2].bypass_actors = p.rulesets[0].bypass_actors; },
  "missing review": (p) => { p.rulesets[2].rules[0].parameters.required_approving_review_count = 0; },
  "missing required check": (p) => { p.rulesets[2].rules[3].parameters.required_status_checks.pop(); },
  "missing reviewers": (p) => { p.environment.protection_rules = []; },
  "self review": (p) => { p.environment.protection_rules[0].prevent_self_review = false; },
  "tag deployment policy": (p) => { p.deployments.branch_policies = [{ name: "v*", type: "tag" }]; },
  "extra deployment ref": (p) => { p.deployments.branch_policies.push({ name: "*", type: "branch" }); },
  "different default branch": (p) => { p.repository.default_branch = "unreviewed"; },
};
for (const [name, mutate] of Object.entries(invalidPolicies)) {
  test(`reject ${name}`, () => {
    const policy = fixture();
    mutate(policy);
    assert.throws(() => verifyRepositoryPolicy(policy), /リリース保護設定が不足/);
  });
}
