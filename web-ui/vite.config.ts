import { defineConfig } from "vite";

export default defineConfig({
  base: "./",
  build: {
    outDir: "dist",
    // 极小产物：单页、无框架运行时
    rollupOptions: { output: { manualChunks: undefined } },
  },
  server: {
    // 边缘联调：vite dev 代理到本机 cloudmountd
    proxy: {
      "/api": "http://127.0.0.1:8384",
    },
  },
});
