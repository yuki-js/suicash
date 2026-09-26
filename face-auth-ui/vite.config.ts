import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    // The AOSP WebView on Hi-CARA (Android 10) is Chromium 74.
    // Pin the target so optional chaining etc. get transpiled.
    target: "chrome74",
    cssTarget: "chrome74",
  },
});
