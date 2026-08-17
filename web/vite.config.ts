import { defineConfig } from "vite";

const backend = process.env.NEURALNAV_API ?? "http://localhost:4173";

export default defineConfig({
  server: {
    port: 5173,
    proxy: {
      "/api": { target: backend, changeOrigin: true },
      "/health": { target: backend, changeOrigin: true },
    },
  },
  build: { outDir: "dist", sourcemap: false },
});
