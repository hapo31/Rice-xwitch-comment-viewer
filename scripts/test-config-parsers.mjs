import assert from "node:assert/strict";
import { test } from "node:test";
import { parseToml, parseYaml } from "./config-parsers.mjs";

test("YAML accepts comments, quoting and ordering while rejecting duplicate keys", () => {
  const parsed = parseYaml("# comment\nsecond: 'value'\nfirst: \"plain\" # inline\n");
  assert.deepEqual(parsed, { second: "value", first: "plain" });
  assert.throws(() => parseYaml("permission: read\npermission: write\n"), /Invalid YAML/);
});

test("YAML aliases are bounded during object construction", () => {
  const aliases = ["base: &base [x]", ...Array.from({ length: 55 }, (_, index) => `alias${index}: *base`)].join("\n");
  assert.throws(() => parseYaml(aliases), /Invalid YAML/);
});

test("TOML accepts equivalent quoting and ordering and rejects malformed or duplicate values", () => {
  assert.deepEqual({ ...parseToml("second = 'value' # comment\nfirst = \"plain\"\n") }, { second: "value", first: "plain" });
  assert.throws(() => parseToml("license = \"MIT\"\nlicense = 'MIT'\n"), /Invalid TOML/);
  assert.throws(() => parseToml("[policy\n"), /Invalid TOML/);
});
