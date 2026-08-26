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

requireMatch(
  publishWorkflow,
  /workflow_run:/,
  "publish workflow は default branch の workflow_run から起動してください。",
);
requireMatch(
  publishWorkflow,
  /environment:\s*\n\s+name:\s*release/,
  "publish job は protected release environment を使用する必要があります。",
);
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

console.log("release workflow policy checks passed");
