import { defineConfig } from "vite";

// Tauri expects a fixed dev port and does not want Vite clearing its output.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "es2021",
    // Keep source maps out of release bundles for a leaner artifact.
    sourcemap: false,
  },
});
