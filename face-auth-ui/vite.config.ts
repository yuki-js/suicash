import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    // Hi-CARA (Android 10) の AOSP WebView は Chromium 74。
    // オプショナルチェーン等をトランスパイルするためターゲットを固定する。
    target: "chrome74",
    cssTarget: "chrome74",
  },
});
