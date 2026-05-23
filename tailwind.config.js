/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        zinc: {
          850: "#1b1b20",
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
