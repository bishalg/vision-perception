import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 3000,
    open: false,
    // CurvStudio Python API (scripts/studio_server.py, default :8080).
    // Override with VITE_STUDIO_API=http://127.0.0.1:<port> (studio:dev sets it).
    proxy: (() => {
      const apiTarget = process.env.VITE_STUDIO_API ?? 'http://127.0.0.1:8080';
      const proxyPaths = ['/api', '/data', '/output'];
      return Object.fromEntries(
        proxyPaths.map(p => [p, { target: apiTarget, changeOrigin: true }]),
      );
    })(),
  },
  build: {
    outDir: 'dist',
    sourcemap: true,
    rollupOptions: {
      output: {
        entryFileNames: 'assets/[name].js',
        chunkFileNames: 'assets/[name].js',
        assetFileNames: 'assets/[name].[ext]',
      },
    },
  },
});
