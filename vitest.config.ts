import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    maxWorkers: 2,
    dangerouslyIgnoreUnhandledErrors: false,
    projects: [
      {
        extends: true,
        test: {
          name: "unit",
          environment: "node",
          include: ["src/**/*.test.{ts,tsx}"],
          exclude: ["src/**/*.dom.test.{ts,tsx}"],
        },
      },
      {
        extends: true,
        test: {
          name: "dom",
          environment: "jsdom",
          include: ["src/**/*.dom.test.{ts,tsx}"],
          setupFiles: ["src/testing/setup.ts"],
          testTimeout: 10000,
          hookTimeout: 10000,
        },
      },
    ],
  },
});
