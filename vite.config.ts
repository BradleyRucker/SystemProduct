import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [sveltekit()],

  // Tauri-specific config
  clearScreen: false,
  server: {
    host: host || false,
    port: 1420,
    strictPort: true,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Watch Rust source for the sidebar indicator
      ignored: ["**/src-tauri/target/**"],
    },
  },

  // Prevent vite from obscuring Rust errors
  build: {
    target:
      process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },

  // Monaco editor must not be pre-bundled — it manages its own web workers
  optimizeDeps: {
    exclude: ["monaco-editor"],
  },
});
