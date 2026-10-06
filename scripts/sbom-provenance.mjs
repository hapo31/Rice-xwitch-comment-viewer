import { createHash } from "node:crypto";
import { basename } from "node:path";
import { packagePurl } from "./sbom-purl.mjs";

const checksum = (content) => createHash("sha256").update(content).digest("hex");
const digest = (value) => /^[a-f0-9]{64}$/.test(value ?? "");

export function collectSbomProvenance({ materials, manifest, lockfiles, artifactHashes, expectedCommit }) {
  if (materials.schemaVersion !== 1 || !/^[a-f0-9]{40}$/.test(materials.commit ?? "") || materials.commit !== expectedCommit || !Number.isInteger(materials.sourceDateEpoch)) throw new Error("Build materials must match the exact source commit");
  for (const kind of ["npm", "cargo"]) if (!digest(materials.lockfiles?.[kind]) || materials.lockfiles[kind] !== checksum(lockfiles[kind])) throw new Error(`Build lockfile mismatch: ${kind}`);
  if (!Array.isArray(materials.artifacts) || !materials.artifacts.some((artifact) => artifact.name.endsWith(".exe")) || !materials.artifacts.some((artifact) => artifact.name.endsWith(".zip"))) throw new Error("Installer and portable artifacts are required");
  if (!Array.isArray(materials.osPackages) || !materials.osPackages.length || !Array.isArray(materials.windowsBuildMaterials) || !materials.windowsBuildMaterials.length) throw new Error("OS and Windows build material inventories are required");

  const components = [], rootRefs = new Set();
  const add = (component) => { components.push(component); rootRefs.add(component.ref); };
  for (const image of [materials.inputs?.rustImage, materials.inputs?.nodeImage]) {
    if (!/@sha256:[a-f0-9]{64}$/.test(image ?? "")) throw new Error("Build image digest missing");
    add({ ref: `build-image:${image}`, type: "container", name: image.split("@")[0], scope: "excluded", hashes: [image.split("@sha256:")[1]], properties: [["role", "build-image"]] });
  }
  for (const line of materials.osPackages) {
    const [name, version, ...extra] = line.split("\t");
    if (!name || !version || extra.length) throw new Error("Invalid installed OS package inventory");
    const purl = packagePurl("deb", name, version, "debian");
    add({ ref: purl, type: "library", name, version, purl, scope: "excluded", properties: [["role", "build-os-package"]] });
  }
  for (const name of Object.keys(materials.toolHashes ?? {})) {
    if (!Object.hasOwn(materials.tools ?? {}, name) || !digest(materials.toolHashes[name])) throw new Error("Unknown tool or invalid compiler hash");
  }
  if (materials.inputs?.nsisVersion && (!digest(materials.toolHashes?.nsis) || materials.tools?.nsis !== `v${materials.inputs.nsisVersion}`)) throw new Error("Reviewed NSIS compiler version and digest are required");
  for (const [name, version] of Object.entries(materials.tools ?? {})) {
    const ref = `build-tool:${name}`;
    const sha256 = materials.toolHashes?.[name];
    add({ ref, type: "application", name, version, scope: "excluded", ...(sha256 ? { hashes: [sha256] } : {}), properties: [["role", "build-tool"]] });
  }
  for (const file of materials.windowsBuildMaterials) {
    if (!digest(file.sha256)) throw new Error("Windows material hash missing");
    add({ ref: `windows-material:${file.path}`, type: "file", name: file.path, scope: "excluded", hashes: [file.sha256], properties: [["role", "windows-sdk-crt-build-input"]] });
  }
  for (const artifact of materials.artifacts) {
    if (basename(artifact.name) !== artifact.name || !digest(artifact.sha256) || artifactHashes[artifact.name] !== artifact.sha256) throw new Error(`Artifact digest mismatch: ${artifact.name}`);
    add({ ref: `artifact:${artifact.name}`, type: "file", name: artifact.name, hashes: [artifact.sha256] });
  }

  return {
    rootRef: `rice:${materials.commit}`,
    rootRefs,
    components,
    rootProperties: [
      ["source-commit", materials.commit],
      ["source-date-epoch", materials.sourceDateEpoch],
      ["npm-lock-sha256", materials.lockfiles.npm],
      ["cargo-lock-sha256", materials.lockfiles.cargo],
    ],
    applicationName: manifest.name,
    applicationVersion: manifest.version,
  };
}

export { checksum, digest };
