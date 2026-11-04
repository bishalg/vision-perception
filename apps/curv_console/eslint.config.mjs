import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";
import curv from "./eslint-rules/no-literal-ui-props.mjs";

/**
 * I18N-9 ratchet: both literal-text rules are `warn` everywhere; a wave package appends the directories (globs) it has
 * made clean here and they become `error`. The integration commit after the wave replaces this with `error` globally.
 */
const LITERAL_GATE_ERROR_DIRS = [
  // L.1a: console shell, sessions, review, devices, audit.
  "src/app/login",
  "src/app/[[]org]/overview",
  "src/app/[[]org]/sessions",
  "src/app/[[]org]/review",
  "src/app/[[]org]/devices",
  "src/app/[[]org]/audit",
  // L.1c exports, privacy, security, settings, share (`[`/`]` are escaped: the entries are globs)
  "src/app/\\[org\\]/exports",
  "src/app/\\[org\\]/erasure",
  "src/app/\\[org\\]/settings",
  "src/app/share",
  "src/app/api",
  "src/components/exports",
  "src/components/privacy",
  "src/components/security",
  // L.1b: participants, encounters, attachments, follow-ups, worklist, clinical review (`[org]` is escaped: it is a glob).
  "src/app/\\[org\\]/participants",
  "src/app/\\[org\\]/worklist",
  "src/app/\\[org\\]/follow-ups",
  "src/components/participant",
  "src/components/clinical",
  // L.1d: users, catalog, config, research (a bracket is a glob class, so the dynamic segment is escaped)
  "src/app/\\[org\\]/users",
  "src/app/\\[org\\]/config",
  "src/app/\\[org\\]/research",
  "src/components/catalog",
  "src/components/config",
  "src/components/research",
];

const literalRules = (level) => ({
  "react/jsx-no-literals": [
    level,
    { noStrings: true, ignoreProps: true, allowedStrings: ["…", "·", "—", "–", "/", "|", ":", "(", ")", "%", "×", "+", "-", "*"] },
  ],
  "curv/no-literal-ui-props": level,
});

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
  {
    // I18N-9: user-visible literals. Exempt a line with `// eslint-disable-next-line react/jsx-no-literals -- i18n-ok: <reason>`.
    files: ["src/**/*.tsx"],
    ignores: ["src/**/*.test.tsx", "src/messages/domain.ts"],
    plugins: { curv },
    rules: literalRules("warn"),
  },
  ...(LITERAL_GATE_ERROR_DIRS.length
    ? [
        {
          files: LITERAL_GATE_ERROR_DIRS.map((d) => `${d}/**/*.tsx`),
          ignores: ["src/**/*.test.tsx"],
          plugins: { curv },
          rules: literalRules("error"),
        },
      ]
    : []),
  {
    // L.1a: single files of the shell that share a directory with other packages (components, org layout and error pages).
    files: [
      "src/app/page.tsx",
      "src/app/not-found.tsx",
      "src/app/[[]org]/layout.tsx",
      "src/app/[[]org]/error.tsx",
      "src/app/[[]org]/not-found.tsx",
      ...[
        "AuditFilters", "AuditTable", "ClipsTable", "DevicesTable", "Distribution", "DriftChart", "EnrolCodePanel", "FlagBadge",
        "KpiCard", "LossBar", "NavLinks", "OrgSwitcher", "Pagination", "QualityPanel", "ReportPanel", "ReviewQueue",
        "ScoringStatus", "SessionFilters", "SessionsTable", "SessionTabs", "TrackingQuality",
      ].map((c) => `src/components/${c}.tsx`),
    ],
    plugins: { curv },
    rules: literalRules("error"),
  },
  globalIgnores([".next/**", "out/**", "coverage/**", "next-env.d.ts", "public/asd-wasm/**"]),
]);
