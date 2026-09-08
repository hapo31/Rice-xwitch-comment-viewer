#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { pathToFileURL } from "node:url";

// Single-maintainer policy: no team, second reviewer or environment is required.
// The trusted workflow and tag verifier both use main as their source boundary.
export function verifyRepositoryPolicy({ repository }) {
  if (repository.default_branch !== "main") {
    throw new Error("リリース元の default branch を main にしてください。");
  }
}
export function fetchRepositoryPolicy(repositoryName, execute = execFileSync) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repositoryName) || repositoryName.split("/").some((part) => part === "." || part === "..")) {
    throw new Error("repository は owner/name 形式で指定してください。");
  }
  return { repository: JSON.parse(execute("gh", ["api", `repos/${repositoryName}`], { encoding: "utf8" })) };
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    verifyRepositoryPolicy(fetchRepositoryPolicy(process.argv[2] ?? ""));
    console.log("release repository policy checks passed (single maintainer)");
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
