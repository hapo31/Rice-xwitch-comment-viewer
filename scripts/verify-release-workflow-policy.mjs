#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const buildWorkflow = readFileSync(
  resolve(root, ".github/workflows/release-windows.yml"),
  "utf8",
);
const publishWorkflow = readFileSync(
  resolve(root, ".github/workflows/publish-windows-release.yml"),
  "utf8",
);

function requireMatch(source, pattern, message) {
  if (!pattern.test(source)) {
    throw new Error(message);
  }
}

if (/contents:\s*write/.test(buildWorkflow)) {
  throw new Error(
    "tag push で起動する release-windows.yml に contents: write を付与しないでください。",
  );
}

requireMatch(
  buildWorkflow,
  /--expected-commit\s+"\$\{GITHUB_SHA\}"/,
  "build workflow は tag target と event GITHUB_SHA を照合する必要があります。",
);
requireMatch(
  buildWorkflow,
  /--checkout-ref\s+HEAD/,
  "build workflow は tag target と checkout HEAD を照合する必要があります。",
);
requireMatch(
  buildWorkflow,
  /--main-ref\s+refs\/remotes\/origin\/main/,
  "build workflow は tag target が origin/main 上にあることを検証する必要があります。",
);
requireMatch(
  buildWorkflow,
  /rice-release-provenance-/,
  "build workflow は tag object の provenance artifact を保存する必要があります。",
);
requireMatch(buildWorkflow, /windows-tests:\s*\n\s+name: Windows production tests\s*\n\s+uses: \.\/\.github\/workflows\/single-instance\.yml/, "release must run the complete Windows production tests");
requireMatch(buildWorkflow, /windows-smoke:\s*\n\s+name: Smoke exact Windows release artifacts\s*\n\s+needs: \[build-windows, windows-tests\]/, "exact artifact smoke must require the build and Windows tests");
requireMatch(buildWorkflow, /node scripts\/verify-release-artifacts\.mjs release-artifacts --write/, "release must inspect exact artifacts and ZIP integrity before upload");
requireMatch(buildWorkflow, /\.\/scripts\/smoke-windows-artifacts\.ps1 -Artifacts release-artifacts -Commit \$env:GITHUB_SHA/, "release must execute the installer and portable on Windows");

requireMatch(
  publishWorkflow,
  /workflow_run:/,
  "publish workflow は default branch の workflow_run から起動してください。",
);
requireMatch(
  publishWorkflow,
  /needs:\s*repository-policy/,
  "publish job は read-only repository-policy job の成功を必須にしてください。",
);
requireMatch(
  publishWorkflow,
  /node trusted\/scripts\/verify-release-repository-policy\.mjs/,
  "default branch を公開直前にも確認してください。",
);
if (/environment:/.test(publishWorkflow)) {
  throw new Error("単独管理の公開に environment 承認を必須にしないでください。");
}
requireMatch(publishWorkflow, /sha256sum --check --strict SHA256SUMS.txt/, "公開前に取得した成果物のchecksumを検証してください。");
requireMatch(
  publishWorkflow,
  /permissions:\s*\n\s+actions:\s*read\s*\n\s+contents:\s*write/,
  "contents: write は publish job だけへ付与してください。",
);
requireMatch(
  publishWorkflow,
  /ref:\s*\$\{\{ github\.sha \}\}\s*\n\s+path:\s*trusted/,
  "publish workflow は workflow_run の default-branch SHA から trusted policy を checkout する必要があります。",
);
requireMatch(
  publishWorkflow,
  /\.\.\/trusted\/scripts\/verify-release-tag\.sh/,
  "publish workflow は tag commit ではなく trusted tag verifier を実行してください。",
);
requireMatch(
  publishWorkflow,
  /--expected-tag-object\s+"\$\{provenance_tag_object\}"/,
  "publish workflow は build 時の tag object と current tag を照合する必要があります。",
);
requireMatch(
  publishWorkflow,
  /run-id:\s*\$\{\{ github\.event\.workflow_run\.id \}\}/,
  "publish workflow は検証対象 run の artifact だけを取得する必要があります。",
);
requireMatch(publishWorkflow, /name: rice-windows-\$\{\{ github\.event\.workflow_run\.id \}\}/, "published bundle must come from the exact tested run");
requireMatch(publishWorkflow, /name: rice-windows-smoke-\$\{\{ github\.event\.workflow_run\.id \}\}/, "publisher must fetch the Windows smoke receipt from the same run");
requireMatch(publishWorkflow, /node trusted\/scripts\/verify-windows-smoke\.mjs/, "trusted publication policy must verify actual Windows jobs and artifact-bound receipt");
if (publishWorkflow.indexOf("node trusted/scripts/verify-windows-smoke.mjs") >= publishWorkflow.indexOf("../trusted/scripts/publish-release.sh")) throw new Error("Windows smoke must be verified before any Release mutation");

console.log("release workflow policy checks passed");
