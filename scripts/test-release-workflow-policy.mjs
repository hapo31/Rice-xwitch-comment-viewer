import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parseYaml } from "./config-parsers.mjs";
import { validateReleaseWorkflows } from "./verify-release-workflow-policy.mjs";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const buildSource = read(".github/workflows/release-windows.yml");
const publishSource = read(".github/workflows/publish-windows-release.yml");
const load = () => [parseYaml(buildSource), parseYaml(publishSource)];

test("structured release policy accepts the repository workflows", () => {
  const [build, publish] = load();
  assert.equal(validateReleaseWorkflows(build, publish), true);
});

test("comments, equivalent quoting and workflow key order do not change policy results", () => {
  const build = parseYaml(`${buildSource}\n# contents: write is intentionally absent from this workflow.\n`);
  const publish = parseYaml(publishSource.replace("contents: read", 'contents: "read"'));
  build.jobs = Object.fromEntries(Object.entries(build.jobs).reverse());
  assert.equal(validateReleaseWorkflows(build, publish), true);
});

test("tag-triggered permissions cannot grant contents write", () => {
  const [build, publish] = load();
  build.jobs["build-windows"].permissions.contents = "write";
  assert.throws(() => validateReleaseWorkflows(build, publish), /permissions must stay/);
});

test("unknown workflow jobs and additional permission scopes require an explicit policy update", () => {
  for (const mutate of [
    (build) => { build.jobs["build-windows"].permissions.actions = "write"; },
    (build) => { build.jobs.unreviewed = { permissions: { contents: "write" } }; },
  ]) {
    const [build, publish] = load();
    mutate(build);
    assert.throws(() => validateReleaseWorkflows(build, publish), /permissions must stay|unknown or missing job/);
  }
});

test("required Windows jobs and their dependency order cannot be removed", () => {
  for (const mutate of [
    (build) => delete build.jobs["windows-tests"],
    (build) => { build.jobs["windows-smoke"].needs = ["build-windows"]; },
  ]) {
    const [build, publish] = load();
    mutate(build);
    assert.throws(() => validateReleaseWorkflows(build, publish), /unknown or missing job|include job|Windows production tests|exact artifact smoke/);
  }
});

test("publication write access, environment approval and smoke ordering stay constrained", () => {
  const cases = [
    (publish) => { publish.jobs["repository-policy"].permissions.contents = "write"; },
    (publish) => { publish.jobs.release.permissions["pull-requests"] = "write"; },
    (publish) => { publish.jobs.release.environment = "production"; },
    (publish) => {
      const steps = publish.jobs.release.steps;
      const verifier = steps.findIndex((step) => step.run?.includes("verify-windows-smoke.mjs"));
      const mutation = steps.findIndex((step) => step.run?.includes("publish-release.sh"));
      [steps[verifier], steps[mutation]] = [steps[mutation], steps[verifier]];
    },
  ];
  for (const mutate of cases) {
    const [build, publish] = load();
    mutate(publish);
    assert.throws(() => validateReleaseWorkflows(build, publish));
  }
});
