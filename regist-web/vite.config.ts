import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  // 相対パスで出力する。任意の静的ホスト(サブパス配下でも可)へそのまま置ける。
  base: "./",
  plugins: [react()],
  preview: {
    port: 1919,
    host: true,
  },
});
