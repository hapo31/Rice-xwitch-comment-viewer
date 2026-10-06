import { PackageURL } from "packageurl-js";

export function packagePurl(type, name, version, namespace) {
  if (typeof name !== "string" || !name || typeof version !== "string" || !version) throw new Error("Package URL name and version are required");
  if (type === "npm") {
    if (name.startsWith("@")) {
      const match = /^(@[^/]+)\/([^/]+)$/.exec(name);
      if (!match) throw new Error(`Invalid scoped npm package name: ${name}`);
      return new PackageURL("npm", match[1], match[2], version).toString();
    }
    if (name.includes("/")) throw new Error(`Invalid npm package name: ${name}`);
    return new PackageURL("npm", undefined, name, version).toString();
  }
  return new PackageURL(type, namespace, name, version).toString();
}

export function parsePackagePurl(value) {
  return PackageURL.fromString(value);
}
