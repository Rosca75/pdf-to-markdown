import { defineConfig } from "vitest/config";

// Tauri expects a fixed dev port and a modern WebView target.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
    watch: { ignored: ["**/src-tauri/**", "**/crates/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    outDir: "dist",
    target:
      process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    environment: "jsdom",
    pool: "threads",
    include: ["src/**/*.test.ts"],
  },
});
