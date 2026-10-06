import { readFileSync } from "node:fs";
import { parseDocument } from "yaml";
import { parse as parseTomlDocument } from "smol-toml";

const MAX_YAML_ALIASES = 50;

export function parseYaml(source, label = "YAML") {
  const document = parseDocument(source, { uniqueKeys: true, version: "1.2" });
  if (document.errors.length) {
    throw new Error(`Invalid ${label}: ${document.errors.map((error) => error.message).join("; ")}`);
  }
  if (document.warnings.length) {
    throw new Error(`Unsupported ${label}: ${document.warnings.map((warning) => warning.message).join("; ")}`);
  }
  try {
    return document.toJS({ maxAliasCount: MAX_YAML_ALIASES });
  } catch (error) {
    throw new Error(`Invalid ${label}: ${error.message}`, { cause: error });
  }
}

export function parseToml(source, label = "TOML") {
  try {
    return parseTomlDocument(source);
  } catch (error) {
    throw new Error(`Invalid ${label}: ${error.message}`, { cause: error });
  }
}

export function readYaml(path) {
  return parseYaml(readFileSync(path, "utf8"), path);
}

export function readToml(path) {
  return parseToml(readFileSync(path, "utf8"), path);
}
