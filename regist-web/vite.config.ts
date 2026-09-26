import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  // Emit relative paths so the output can be dropped onto any static host (subpaths too).
  base: "./",
  plugins: [react()],
  preview: {
    port: 1919,
    host: true,
    // Disable the host check so any hostname behind the reverse proxy (automatic
    // Let's Encrypt) works. vite preview is a simple server; a separate TLS proxy
    // sits in front of it.
    allowedHosts: true,
  },
});
