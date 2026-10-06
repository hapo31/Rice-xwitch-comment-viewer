#!/usr/bin/env node

import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { readYaml } from "./config-parsers.mjs";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));

function requirePolicy(condition, message) {
  if (!condition) throw new Error(message);
}

function stepsFor(job, label) {
  requirePolicy(job && Array.isArray(job.steps), `${label} must declare steps`);
  return job.steps;
}

function runText(steps) {
  return steps.map((step) => step?.run).filter((run) => typeof run === "string").join("\n");
}

function requireExactMap(actual, expected, message) {
  requirePolicy(actual && typeof actual === "object" && !Array.isArray(actual), message);
  const actualKeys = Object.keys(actual).sort();
  const expectedKeys = Object.keys(expected).sort();
  requirePolicy(actualKeys.length === expectedKeys.length && actualKeys.every((key, index) => key === expectedKeys[index] && actual[key] === expected[key]), message);
}

function requireExactKeys(actual, expected, message) {
  requirePolicy(actual && typeof actual === "object" && !Array.isArray(actual), message);
  const actualKeys = Object.keys(actual).sort();
  const expectedKeys = [...expected].sort();
  requirePolicy(actualKeys.length === expectedKeys.length && actualKeys.every((key, index) => key === expectedKeys[index]), message);
}

function requireJob(workflow, name, label) {
  const job = workflow.jobs?.[name];
  requirePolicy(job && typeof job === "object", `${label} must include job ${name}`);
  return job;
}

function requireExactNeeds(job, expected, message) {
  const needs = Array.isArray(job.needs) ? job.needs : [job.needs];
  requirePolicy(needs.length === expected.length && expected.every((name) => needs.includes(name)), message);
}

function requireVerifierDependencies(steps, invocation, directory) {
  const invokeAt = steps.findIndex((step) => typeof step?.run === "string" && invocation.test(step.run));
  const installAt = steps.findIndex((step) =>
    step?.if === undefined && step?.["continue-on-error"] !== true &&
    (step?.["working-directory"] ?? ".") === directory &&
    typeof step?.run === "string" &&
    /^\s*corepack pnpm install --frozen-lockfile --ignore-scripts\s*$/m.test(step.run));
  requirePolicy(installAt >= 0 && invokeAt > installAt, `SBOM verifier dependencies must be installed from ${directory} before verification`);
  requirePolicy(steps.slice(0, installAt).some((step) => step?.uses?.startsWith("actions/setup-node@")), "SBOM verifier must select a compatible Node version before dependency installation");
}

export function validateReleaseWorkflows(buildWorkflow, publishWorkflow) {
  requirePolicy(buildWorkflow && typeof buildWorkflow === "object" && buildWorkflow.jobs, "release build workflow must contain jobs");
  requirePolicy(publishWorkflow && typeof publishWorkflow === "object" && publishWorkflow.jobs, "release publisher workflow must contain jobs");
  requireExactKeys(buildWorkflow.jobs, ["quality", "windows-tests", "build-windows", "windows-smoke"], "release build contains an unknown or missing job");
  requireExactMap(buildWorkflow.permissions, { contents: "read" }, "tag-triggered release permissions must stay contents: read only");
  for (const [jobName, job] of Object.entries(buildWorkflow.jobs)) {
    if (job.permissions !== undefined) requireExactMap(job.permissions, { contents: "read" }, `release job ${jobName} permissions must stay contents: read only`);
  }

  const build = requireJob(buildWorkflow, "build-windows", "release build");
  const windowsTests = requireJob(buildWorkflow, "windows-tests", "release build");
  requirePolicy(windowsTests.name === "Windows production tests" && windowsTests.uses === "./.github/workflows/single-instance.yml", "release must run the complete Windows production tests");
  const smoke = requireJob(buildWorkflow, "windows-smoke", "release build");
  requireExactNeeds(smoke, ["build-windows", "windows-tests"], "exact artifact smoke must require the build and Windows tests");
  const buildSteps = stepsFor(build, "build-windows");
  const buildCommands = runText(buildSteps);
  const smokeSteps = stepsFor(smoke, "windows-smoke");
  const smokeCommands = runText(smokeSteps);
  requirePolicy(buildSteps.some((step) => step?.uses?.startsWith("actions/upload-artifact@") && step.with?.name === "rice-release-provenance-${{ github.ref_name }}" && step.with?.path === "release-provenance.json"), "build workflow must retain the tag object provenance artifact");
  for (const [pattern, message] of [
    [/--expected-commit\s+"\$\{GITHUB_SHA\}"/, "build workflow must compare the tag target with event GITHUB_SHA"],
    [/--checkout-ref\s+HEAD/, "build workflow must compare the tag target with checkout HEAD"],
    [/--main-ref\s+refs\/remotes\/origin\/main/, "build workflow must verify the tag target is on origin/main"],
    [/node scripts\/verify-release-artifacts\.mjs release-artifacts --write/, "release must inspect exact artifacts and ZIP integrity before upload"],
  ]) requirePolicy(pattern.test(buildCommands), message);
  requireVerifierDependencies(smokeSteps, /smoke-windows-artifacts\.ps1/, ".");
  requirePolicy(/\.\/scripts\/smoke-windows-artifacts\.ps1 -Artifacts release-artifacts -Commit \$env:GITHUB_SHA/.test(smokeCommands), "release must execute the installer and portable on Windows");

  const publisherTrigger = publishWorkflow.on?.workflow_run;
  requirePolicy(publisherTrigger && typeof publisherTrigger === "object", "publish workflow must be triggered by workflow_run");
  const repositoryPolicy = requireJob(publishWorkflow, "repository-policy", "publish workflow");
  const release = requireJob(publishWorkflow, "release", "publish workflow");
  requireExactKeys(publishWorkflow.jobs, ["repository-policy", "release"], "publish workflow contains an unknown or missing job");
  requireExactNeeds(release, ["repository-policy"], "publish job must require the read-only repository-policy job");
  requireExactMap(publishWorkflow.permissions, { contents: "read" }, "publish workflow must have read-only default permissions");
  requireExactMap(repositoryPolicy.permissions, { actions: "read", contents: "read" }, "repository-policy job must remain read-only");
  requireExactMap(release.permissions, { actions: "read", contents: "write" }, "publish permissions may grant only actions: read and contents: write");
  requirePolicy(publishWorkflow.environment === undefined && Object.values(publishWorkflow.jobs).every((job) => job.environment === undefined), "single-owner publication must not require environment approval");
  const publisherSteps = stepsFor(release, "release");
  const trustedCheckout = publisherSteps.some((step) => typeof step?.uses === "string" && step.uses.startsWith("actions/checkout@") && step.with?.ref === "${{ github.sha }}" && step.with?.path === "trusted");
  requirePolicy(trustedCheckout, "publish workflow must checkout trusted policy from the workflow_run default-branch SHA");
  requireVerifierDependencies(publisherSteps, /node trusted\/scripts\/verify-windows-smoke\.mjs/, "trusted");
  const commands = runText(publisherSteps);
  const bundleDownload = publisherSteps.some((step) => step?.uses?.startsWith("actions/download-artifact@") && step.with?.name === "rice-windows-${{ github.event.workflow_run.id }}" && step.with?.["run-id"] === "${{ github.event.workflow_run.id }}");
  const smokeDownload = publisherSteps.some((step) => step?.uses?.startsWith("actions/download-artifact@") && step.with?.name === "rice-windows-smoke-${{ github.event.workflow_run.id }}" && step.with?.["run-id"] === "${{ github.event.workflow_run.id }}");
  requirePolicy(bundleDownload, "published bundle must come from the exact tested run");
  requirePolicy(smokeDownload, "publisher must fetch the Windows smoke receipt from the same run");
  for (const [pattern, message] of [
    [/node trusted\/scripts\/verify-release-repository-policy\.mjs/, "default branch must be checked immediately before publication"],
    [/sha256sum --check --strict SHA256SUMS\.txt/, "downloaded artifacts must pass checksum verification"],
    [/\.\.\/trusted\/scripts\/verify-release-tag\.sh/, "publisher must use the trusted tag verifier"],
    [/--expected-tag-object\s+"\$\{provenance_tag_object\}"/, "publisher must compare the build tag object with the current tag"],
    [/node trusted\/scripts\/verify-windows-smoke\.mjs/, "trusted publication policy must verify actual Windows jobs and artifact-bound receipt"],
  ]) requirePolicy(pattern.test(commands), message);
  const smokeVerificationIndex = publisherSteps.findIndex((step) => typeof step?.run === "string" && /node trusted\/scripts\/verify-windows-smoke\.mjs/.test(step.run));
  const publishMutationIndex = publisherSteps.findIndex((step) => typeof step?.run === "string" && /\.\.\/trusted\/scripts\/publish-release\.sh/.test(step.run));
  requirePolicy(smokeVerificationIndex >= 0 && publishMutationIndex > smokeVerificationIndex, "Windows smoke must be verified before any Release mutation");
  return true;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  validateReleaseWorkflows(readYaml(resolve(root, ".github/workflows/release-windows.yml")), readYaml(resolve(root, ".github/workflows/publish-windows-release.yml")));
  console.log("release workflow policy checks passed");
}
