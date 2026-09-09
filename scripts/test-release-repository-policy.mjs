import assert from "node:assert/strict";
import { test } from "node:test";
import { fetchRepositoryPolicy, verifyRepositoryPolicy } from "./verify-release-repository-policy.mjs";

test("single-maintainer main needs no team, ruleset or reviewer", () => {
  verifyRepositoryPolicy({ repository: { default_branch: "main" } });
});
test("reject a different or missing default branch", () => {
  for (const repository of [{ default_branch: "other" }, {}]) {
    assert.throws(() => verifyRepositoryPolicy({ repository }), /main/);
  }
});
test("only the repository read endpoint is needed", () => {
  verifyRepositoryPolicy(fetchRepositoryPolicy("owner/repo", (command, args) => {
    assert.equal(command, "gh");
    assert.deepEqual(args, ["api", "repos/owner/repo"]);
    return JSON.stringify({ default_branch: "main" });
  }));
});
test("API errors and malformed repository names fail closed", () => {
  assert.throws(() => fetchRepositoryPolicy("owner/repo", () => { throw new Error("HTTP 403"); }), /403/);
  assert.throws(() => fetchRepositoryPolicy("../bad"), /owner\/name/);
});
