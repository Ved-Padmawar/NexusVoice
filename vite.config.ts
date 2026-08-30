/// <reference types="vitest/config" />
import { defineConfig, withFilter } from "vite";
import { resolve } from "path";
import react from "@vitejs/plugin-react";
import svgr from "vite-plugin-svgr";
import tailwindcss from "@tailwindcss/vite";
import path from "path";
import { readFileSync } from "fs";

const pkg = JSON.parse(readFileSync("./package.json", "utf-8")) as { version: string };

// https://vite.dev/config/
export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  // svgr: provider marks import as components so they can be tinted.
  plugins: [
    react(),
    withFilter(svgr(), { load: { id: /\.svg\?react$/ } }),
    tailwindcss({ optimize: false }),
  ],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  build: {
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, 'index.html'),
        pill: resolve(import.meta.dirname, 'pill.html'),
      },
    },
  },
  // Tests live under `src/__tests__/`, organized by domain, mirroring the backend
  // `src-tauri/tests/unit/`.
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/__tests__/**/*.{test,spec}.{ts,tsx}"],
  },
});
