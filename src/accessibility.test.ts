import { describe, expect, it } from "vitest";

const normalTextColor = "#a1a1aa"; // Tailwind zinc-400
const textBackgrounds = ["#09090b", "#18181b", "#1b1b20"]; // zinc-950, 900, 850

function relativeLuminance(hex: string): number {
  const channels = hex.match(/[a-f\d]{2}/gi)?.map((channel) => Number.parseInt(channel, 16) / 255);
  if (!channels || channels.length !== 3) throw new Error(`Invalid color: ${hex}`);

  return channels.reduce((sum, channel, index) => {
    const linear = channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
    return sum + linear * [0.2126, 0.7152, 0.0722][index];
  }, 0);
}

function contrastRatio(foreground: string, background: string): number {
  const [lighter, darker] = [relativeLuminance(foreground), relativeLuminance(background)].sort((a, b) => b - a);
  return (lighter + 0.05) / (darker + 0.05);
}

const sourceModules = import.meta.glob<string>("./**/*.{ts,tsx}", {
  eager: true,
  import: "default",
  query: "?raw",
});

describe("normal text accessibility", () => {
  it("uses zinc-400 at 4.5:1 or greater on every app background", () => {
    for (const background of textBackgrounds) {
      expect(contrastRatio(normalTextColor, background)).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("does not use zinc-500 or zinc-600 for normal text", () => {
    const lowContrastText = /(^|[\s"'])text-zinc-(?:500|600)\b/m;
    expect(Object.keys(sourceModules).length).toBeGreaterThan(20);

    for (const [path, source] of Object.entries(sourceModules)) {
      if (!path.endsWith(".test.ts")) expect(source, path).not.toMatch(lowContrastText);
    }
  });
});
