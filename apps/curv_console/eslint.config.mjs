import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";

export default defineConfig([
  ...nextVitals,
  ...nextTs,
  {
    // CSP: `@/lib/zod` sets zod's `jitless` before any schema exists; importing zod directly could bypass it.
    files: ["**/*.{ts,tsx,mts,mjs,js,jsx}"],
    ignores: ["src/lib/zod.ts"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          paths: [{ name: "zod", message: 'Import from "@/lib/zod" (sets jitless for the CSP).' }],
          patterns: [{ group: ["zod/*"], message: 'Import from "@/lib/zod" (sets jitless for the CSP).' }],
        },
      ],
    },
  },
  globalIgnores([".next/**", "out/**", "coverage/**", "next-env.d.ts", "public/asd-wasm/**"]),
]);
