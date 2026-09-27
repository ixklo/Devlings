import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  // Threads, not forks: forked workers time out starting up on a busy machine and the run reports "no tests".
  // `css.include`: the contrast test reads the colour tokens as text (`styles.css?raw`); other CSS stays stubbed.
  // `include`: the app's tests only; the pet generator in scripts/pets has its own (`node --test`).
  test: {
    include: ["src/**/*.test.{ts,tsx}"],
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
    pool: "threads",
    css: { include: [/\/src\/styles\.css/] },
  },
});
