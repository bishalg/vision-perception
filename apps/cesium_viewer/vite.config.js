import { defineConfig } from "vite";
import { viteStaticCopy } from "vite-plugin-static-copy";
import path from "node:path";

export default defineConfig({
  base: "./",
  server: {
    port: 3000,
    open: false,
  },
  plugins: [
    viteStaticCopy({
      targets: [
        {
          src: path.join("node_modules/cesium/Build/Cesium/Workers", "*"),
          dest: "Workers",
        },
        {
          src: path.join("node_modules/cesium/Build/Cesium/ThirdParty", "*"),
          dest: "ThirdParty",
        },
        {
          src: path.join("node_modules/cesium/Build/Cesium/Assets", "*"),
          dest: "Assets",
        },
        {
          src: path.join("node_modules/cesium/Build/Cesium/Widgets", "*"),
          dest: "Widgets",
        },
      ],
    }),
  ],
  define: {
    CESIUM_BASE_URL: JSON.stringify("./"),
  },
});
