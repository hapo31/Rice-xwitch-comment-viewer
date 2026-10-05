/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Keep Rice's previous sRGB palette. The dependency migration is not
        // a color redesign, and the contrast checks depend on these values.
        zinc: {
          50: "#fafafa",
          100: "#f4f4f5",
          200: "#e4e4e7",
          300: "#d4d4d8",
          400: "#a1a1aa",
          500: "#71717a",
          600: "#52525b",
          700: "#3f3f46",
          800: "#27272a",
          850: "#1b1b20",
          900: "#18181b",
          950: "#09090b",
        },
        sky: {
          50: "#f0f9ff", 100: "#e0f2fe", 200: "#bae6fd",
          300: "#7dd3fc", 400: "#38bdf8", 500: "#0ea5e9",
          600: "#0284c7", 700: "#0369a1", 800: "#075985",
          900: "#0c4a6e", 950: "#082f49",
        },
        emerald: {
          50: "#ecfdf5", 100: "#d1fae5", 200: "#a7f3d0",
          300: "#6ee7b7", 400: "#34d399", 500: "#10b981",
          600: "#059669", 700: "#047857", 800: "#065f46",
          900: "#064e3b", 950: "#022c22",
        },
        rose: {
          50: "#fff1f2", 100: "#ffe4e6", 200: "#fecdd3",
          300: "#fda4af", 400: "#fb7185", 500: "#f43f5e",
          600: "#e11d48", 700: "#be123c", 800: "#9f1239",
          900: "#881337", 950: "#4c0519",
        },
        amber: {
          50: "#fffbeb", 100: "#fef3c7", 200: "#fde68a",
          300: "#fcd34d", 400: "#fbbf24", 500: "#f59e0b",
          600: "#d97706", 700: "#b45309", 800: "#92400e",
          900: "#78350f", 950: "#451a03",
        },
      },
      fontFamily: {
        sans: [
          "Inter",
          "Yu Gothic UI",
          "Yu Gothic",
          "Meiryo",
          "Hiragino Sans",
          "Hiragino Kaku Gothic ProN",
          "Noto Sans CJK JP",
          "Noto Sans JP",
          "TakaoGothic",
          "ui-sans-serif",
          "system-ui",
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "sans-serif",
        ],
        mono: [
          "Cascadia Mono",
          "Consolas",
          "Noto Sans Mono CJK JP",
          "Noto Sans CJK JP",
          "Yu Gothic UI",
          "Meiryo",
          "ui-monospace",
          "SFMono-Regular",
          "Menlo",
          "Monaco",
          "Liberation Mono",
          "monospace",
        ],
      },
    },
  },
  plugins: [],
};
