import { readFileSync } from "node:fs";
import postcss from "postcss";
import { beforeAll, describe, expect, it } from "vitest";
import { build } from "vite";

// Compile through the actual Vite configuration, not a second test-only
// Tailwind pipeline. These assertions concern generated CSS; the Windows
// native test additionally checks computed styles and real tile geometry.
let css;

beforeAll(async () => {
  const result = await build({
    configFile: "vite.config.ts",
    logLevel: "silent",
    build: { write: false, minify: false, cssMinify: false },
  });
  const outputs = Array.isArray(result) ? result : [result];
  const styles = outputs.flatMap((output) => {
    if (!("output" in output)) throw new Error("Expected a completed frontend build");
    return output.output.flatMap((asset) =>
      asset.type === "asset" && asset.fileName.endsWith(".css")
        ? [typeof asset.source === "string" ? asset.source : Buffer.from(asset.source).toString()]
        : [],
    );
  });
  expect(styles).toHaveLength(1);
  css = postcss.parse(styles[0]);
}, 30000);

function values(selector, property) {
  const found = [];
  css.walkRules((rule) => {
    if (rule.selector.split(",").map((part) => part.trim()).includes(selector)) {
      rule.walkDecls(property, (declaration) => {
        found.push(declaration.value);
      });
    }
  });
  return found;
}

describe("production stylesheet compatibility", () => {
  it("keeps the dark palette and accessible text and state colors", () => {
    for (const [selector, property, expected] of [
      [".bg-zinc-950", "background-color", "#09090b"],
      [".bg-zinc-900", "background-color", "#18181b"],
      [".bg-zinc-850", "background-color", "#1b1b20"],
      [".text-zinc-400", "color", "#a1a1aa"],
      [".text-sky-400", "color", "#38bdf8"],
      [".text-emerald-400", "color", "#34d399"],
      [".text-rose-400", "color", "#fb7185"],
      [".text-amber-400", "color", "#fbbf24"],
    ]) {
      expect(values(selector, property), selector).toContain(expected);
    }
  });

  it("keeps the existing Japanese sans and monospace fallback families", () => {
    const sans = values("body", "font-family").join(" ");
    const mono = values(".font-mono", "font-family").join(" ");
    for (const family of ["Inter", "Yu Gothic UI", "Meiryo", "Noto Sans JP"]) {
      expect(sans).toContain(family);
    }
    for (const family of ["Cascadia Mono", "Consolas", "Noto Sans Mono CJK JP", "Meiryo"]) {
      expect(mono).toContain(family);
    }
  });

  it("emits the icon, grid and text metrics used by the real application", () => {
    expect(values(".h-16", "height")).not.toHaveLength(0);
    expect(values(".w-16", "width")).not.toHaveLength(0);
    expect(values(".auto-rows-\\[156px\\]", "grid-auto-rows")).toContain("156px");
    expect(values(".text-sm", "font-size")).not.toHaveLength(0);
    expect(values(".text-xs", "line-height")).not.toHaveLength(0);
    expect(values(".rounded-sm", "border-radius")).not.toHaveLength(0);
    expect(values(".shadow-xs", "--tw-shadow").join(" ")).toContain("0 1px 2px 0");
  });

  it("keeps keyboard focus rings and the system-color outline fallback", () => {
    expect(values(".focus-visible\\:ring-2:focus-visible", "--tw-ring-shadow").join(" ")).toContain("2px");
    expect(values(".focus-visible\\:ring-sky-400:focus-visible", "--tw-ring-color")).toContain("#38bdf8");
    expect(css.toString()).toContain("forced-colors: active");
    expect(css.toString()).toContain("Highlight");
    expect(values(".outline-hidden", "outline").join(" ")).toContain("2px");
  });

  it("preserves form cursor and placeholder affordances", () => {
    expect(values("button:not(:disabled)", "cursor")).toContain("pointer");
    expect(values("input::placeholder", "color")).toContain("#9ca3af");
    expect(values("input::placeholder", "opacity")).toContain("1");
  });

  it("removes the vulnerable build path instead of hiding an audit finding", () => {
    const lock = readFileSync("pnpm-lock.yaml", "utf8");
    const dependencies = JSON.parse(readFileSync("package.json", "utf8")).devDependencies;
    expect(dependencies.tailwindcss).toMatch(/^4\.\d+\.\d+$/);
    expect(dependencies["@tailwindcss/vite"]).toBe(dependencies.tailwindcss);
    expect(lock).toContain(`/@tailwindcss/vite@${dependencies.tailwindcss}`);
    expect(lock).toContain(`/tailwindcss@${dependencies.tailwindcss}:`);
    expect(lock).not.toMatch(/^  \/(?:braces|micromatch|chokidar|fast-glob)@/m);
    const exceptions = JSON.parse(readFileSync("security/advisory-exceptions.json", "utf8"));
    expect(exceptions.exceptions.some((item) => item.id === "GHSA-vfj7-8cjw-p6xm")).toBe(false);
  });

  it("requests a compatible WebView2 instead of leaving the installer on an old runtime", () => {
    const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
    expect(config.bundle.windows.minimumWebview2Version).toBe("111.0.0.0");
    expect(readFileSync("README.md", "utf8")).toContain("Chromium 111");
  });
});
