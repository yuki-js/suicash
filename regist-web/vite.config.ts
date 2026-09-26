import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  // 相対パスで出力する。任意の静的ホスト(サブパス配下でも可)へそのまま置ける。
  base: "./",
  plugins: [react()],
  preview: {
    port: 1919,
    host: true,
    // リバースプロキシ(Let's Encrypt 自動)配下の任意ホスト名でアクセスできるよう
    // ホストチェックを無効化する。vite preview は簡易サーバーで、front には
    // 別途 TLS プロキシが立つ運用のため。
    allowedHosts: true,
  },
});
