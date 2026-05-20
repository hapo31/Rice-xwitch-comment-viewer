import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: [
        "**/.pnpm-store/**",
        "**/dist/**",
        "**/src-tauri/gen/**",
        "**/src-tauri/target/**",
      ],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
});
