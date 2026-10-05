import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const root = new URL("../", import.meta.url);
function biome(command, input) {
  return spawnSync(process.execPath, ["node_modules/@biomejs/biome/bin/biome", command, "--stdin-file-path=src/quality-probe.tsx"], { cwd: root, input, encoding: "utf8" });
}
function lintFixture(input) {
  const directory = mkdtempSync(fileURLToPath(new URL("../src/quality-probes-", import.meta.url)));
  try {
    const fixture = join(directory, "probe.tsx");
    writeFileSync(fixture, input);
    return spawnSync(process.execPath, ["node_modules/@biomejs/biome/bin/biome", "lint", fixture], { cwd: root, encoding: "utf8" });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("PR, main and release reuse every named gate with read-only permissions", () => {
  const pr = read(".github/workflows/pr-quality.yml");
  const release = read(".github/workflows/release-windows.yml");
  const shared = read(".github/workflows/quality-checks.yml");
  assert.match(pr, /pull_request:/);
  assert.match(pr, /push:\s*\n\s*branches: \[main\]/);
  for (const source of [pr, release]) assert.match(source, /uses: \.\/\.github\/workflows\/quality-checks\.yml/);
  assert.match(release, /needs: quality/);
  for (const gate of ["frontend-format", "frontend-lint", "frontend-typecheck", "frontend-test", "frontend-build", "rust-format", "rust-clippy", "rust-test"]) {
    assert.ok(shared.includes(gate), `Missing ${gate}`);
    assert.ok(read("scripts/quality-gate.sh").includes(`${gate})`));
  }
  assert.match(read("scripts/quality-gate.sh"), /--all-targets --all-features -- -D warnings/);
  assert.match(read("scripts/quality-gate.sh"), /cargo test --locked/);
  assert.doesNotMatch(shared, /contents: write|secrets\.|pull_request_target/);
});

test("lint rejects debugger, duplicate keys, unreachable code and unsafe HTML", () => {
  for (const input of ["debugger;", "const x = { a: 1, a: 2 };", "function f() { return 1; return 2; }", "const x = <div dangerouslySetInnerHTML={{__html: 'external'}} />;"]) {
    const result = lintFixture(input);
    assert.equal(result.status, 1, result.stderr);
    assert.match(result.stderr + result.stdout, /lint\/|contents aren't fixed/);
  }
  assert.equal(lintFixture('const message = "安全な文字列";\n').status, 0);
});

test("formatter normalizes whitespace and formatting is idempotent", () => {
  const input = 'const  message={text:"test"}\n';
  const first = biome("format", input);
  assert.equal(first.status, 0, first.stderr);
  assert.notEqual(first.stdout, input);
  assert.equal(biome("format", first.stdout).stdout, first.stdout);
});

test("lint rejects explicit any in mocks and TypeScript enum/namespace declarations", () => {
  for (const [input, rule] of [
    ["let event: any;", "noExplicitAny"],
    ["const settings = {} as any;", "noExplicitAny"],
    ["type Listener = (event: any) => void;", "noExplicitAny"],
    ["type Values = Array<any>;", "noExplicitAny"],
    ["enum State { Idle, Running }", "noEnum"],
    ["const enum State { Idle, Running }", "noConstEnum"],
    ["namespace Model { export type State = string; }", "noNamespace"],
    ["declare namespace Model { type State = string; }", "noNamespace"],
  ]) {
    const result = lintFixture(input);
    assert.equal(result.status, 1, `${input}\n${result.stderr}`);
    assert.ok((result.stderr + result.stdout).includes(rule), result.stderr);
  }
});

test("lint permits typed mocks, named/alias imports and justified type-level any", () => {
  for (const input of [
    'import { type AppSettings as Settings } from "./types"; type Loaded = Settings[];',
    'import { createDomainStores as createStores } from "./stores/domainStores"; const stores = createStores();',
    "type Listener = (event: unknown) => void;",
    "type Identity<T extends any> = T;",
    // Biome accepts generic constraints natively. Conditional inference needs
    // a local, explained exception rather than disabling the rule for a file.
    "// biome-ignore lint/suspicious/noExplicitAny: type-only conditional inference must accept every parameter list\ntype Result<T> = T extends (...args: any[]) => infer R ? R : never;",
  ]) {
    const result = lintFixture(input);
    assert.equal(result.status, 0, result.stderr);
  }
});
