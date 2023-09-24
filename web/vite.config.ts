import { defineConfig } from "vite";

// wasm plugin is needed to load ".wasm" files in the
// generated hackvm package
import wasm from "vite-plugin-wasm";

// topLevelAwait is needed for the wasm plugin to work
// on older browser targets
import topLevelAwait from "vite-plugin-top-level-await";
import react from "@vitejs/plugin-react-swc";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [wasm(), topLevelAwait(), react()],
});
