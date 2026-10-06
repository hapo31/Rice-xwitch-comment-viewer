export function npmInventory(tree) {
  if (!tree?.dependencies || !tree.devDependencies) throw new Error("Installed npm dependency tree is required");
  const packages = new Map(), edges = new Map(), roots = new Set(), pending = [];
  for (const [scope, entries] of [["required", tree.dependencies], ["excluded", tree.devDependencies]]) {
    for (const [name, entry] of Object.entries(entries)) pending.push({ name, entry, scope, parent: null });
  }
  while (pending.length) {
    const { name, entry, scope, parent } = pending.shift();
    if (!entry.version || !entry.path || !/^\d/.test(entry.version)) throw new Error(`Unresolved npm package: ${name}`);
    const ref = `${name}@${entry.version}`;
    if (parent) edges.get(parent).add(ref); else roots.add(ref);
    const previous = packages.get(ref);
    if (previous && (previous.scope === "required" || scope === "excluded")) continue;
    packages.set(ref, { name, version: entry.version, scope, path: entry.path });
    if (!edges.has(ref)) edges.set(ref, new Set());
    for (const [childName, child] of Object.entries(entry.dependencies ?? {})) pending.push({ name: childName, entry: child, scope, parent: ref });
  }
  return { packages, edges, roots };
}

export function cargoInventory(metadata) {
  if (!metadata.resolve?.root || !Array.isArray(metadata.resolve.nodes) || !Array.isArray(metadata.packages)) throw new Error("Resolved Windows Cargo metadata is required");
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const scopes = new Map(), edges = new Map(), pending = [[metadata.resolve.root, "required"]];
  while (pending.length) {
    const [id, scope] = pending.shift();
    if (scopes.get(id) === "required" || scopes.get(id) === scope) continue;
    scopes.set(id, scope);
    const node = nodes.get(id);
    if (!node) throw new Error(`Missing Cargo dependency node: ${id}`);
    const children = [];
    for (const dependency of node.deps) {
      const kinds = dependency.dep_kinds.filter((kind) => kind.kind !== "dev");
      if (!kinds.length) continue;
      const childScope = scope === "required" && kinds.some((kind) => kind.kind === null) ? "required" : "excluded";
      children.push(dependency.pkg); pending.push([dependency.pkg, childScope]);
    }
    edges.set(id, new Set(children));
  }
  return { scopes, edges };
}
