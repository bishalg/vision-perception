/**
 * WP-C.23 (spec 03 §3a, N-4): the one source of truth for console URLs. Route folders under `src/app/[org]/` mirror
 * this tree (clinic, research, config, data, admin, account), and so do the navigation (`lib/nav.ts`), the
 * `next.config.ts` redirects of the old flat paths, server actions, `guard.ts` and the e2e helpers. No page builds
 * `${base}/…` strings by hand.
 *
 * Pure (no `@/` imports, no `server-only`) so the e2e helpers and `next.config.ts` can import it too.
 */

const enc = encodeURIComponent;

/** `/{org}`: the org segment, URL-encoded. */
export function orgBase(org: string): string {
  return `/${enc(org)}`;
}

/** Config documents edited as versions (catalog, normative baselines, scoring profiles). */
export type ConfigDocSegment = "catalog" | "baselines" | "scoring";

/** Every console page under `base` (`/{org}`, already encoded; see `orgBase`). */
export function orgPaths(base: string) {
  const clinic = `${base}/clinic`;
  const research = `${base}/research`;
  const config = `${base}/config`;
  const data = `${base}/data`;
  const admin = `${base}/admin`;
  const account = `${base}/account`;
  return {
    /** `/{org}`: redirects to `clinic.overview`. */
    root: base,
    clinic: {
      root: clinic,
      overview: `${clinic}/overview`,
      worklist: `${clinic}/worklist`,
      participants: `${clinic}/participants`,
      participant: (id: string) => `${clinic}/participants/${enc(id)}`,
      followUps: `${clinic}/follow-ups`,
      review: `${clinic}/review`,
      sessions: `${clinic}/sessions`,
      session: (id: string) => `${clinic}/sessions/${enc(id)}`,
    },
    research: {
      root: research,
      experiments: `${research}/experiments`,
      newExperiment: `${research}/experiments/new`,
      experiment: (id: string) => `${research}/experiments/${enc(id)}`,
      cohorts: `${research}/cohorts`,
      newCohort: `${research}/cohorts/new`,
      cohort: (id: string) => `${research}/cohorts/${enc(id)}`,
      runs: `${research}/runs`,
      run: (id: string) => `${research}/runs/${enc(id)}`,
      runSessions: (id: string) => `${research}/runs/${enc(id)}/sessions`,
    },
    config: {
      root: config,
      catalog: `${config}/catalog`,
      baselines: `${config}/baselines`,
      scoring: `${config}/scoring`,
      sandbox: `${config}/sandbox`,
      bundles: `${config}/bundles`,
      /** Versions list of a config document kind. */
      doc: (segment: ConfigDocSegment) => `${config}/${segment}`,
      docNew: (segment: ConfigDocSegment) => `${config}/${segment}/new`,
      docVersion: (segment: ConfigDocSegment, version: string) => `${config}/${segment}/${enc(version)}`,
    },
    data: {
      root: data,
      exports: `${data}/exports`,
      export: (id: string) => `${data}/exports/${enc(id)}`,
      erasure: `${data}/erasure`,
      retention: `${data}/retention`,
      audit: `${data}/audit`,
    },
    admin: {
      root: admin,
      users: `${admin}/users`,
      devices: `${admin}/devices`,
      integrations: `${admin}/integrations`,
      integration: (id: string) => `${admin}/integrations/${enc(id)}`,
      clinical: `${admin}/clinical`,
    },
    account: {
      root: account,
      security: `${account}/security`,
    },
  };
}

export type OrgPaths = ReturnType<typeof orgPaths>;

/** `orgPaths(orgBase(org))`: for callers that hold the raw org id. */
export function pathsFor(org: string): OrgPaths {
  return orgPaths(orgBase(org));
}

/**
 * N-7: the flat paths before WP-C.23 and where they live now (relative to `/{org}`). `next.config.ts` turns each
 * into a 308 redirect for one release; `:path*` carries detail segments (`sessions/{id}`, `exports/{id}`, …).
 */
export const LEGACY_REDIRECTS: readonly { from: string; to: string; nested: boolean }[] = [
  { from: "overview", to: "clinic/overview", nested: false },
  { from: "worklist", to: "clinic/worklist", nested: false },
  { from: "participants", to: "clinic/participants", nested: true },
  { from: "follow-ups", to: "clinic/follow-ups", nested: false },
  { from: "review", to: "clinic/review", nested: false },
  { from: "sessions", to: "clinic/sessions", nested: true },
  { from: "exports", to: "data/exports", nested: true },
  { from: "erasure", to: "data/erasure", nested: false },
  { from: "settings/retention", to: "data/retention", nested: false },
  { from: "audit", to: "data/audit", nested: false },
  { from: "users", to: "admin/users", nested: false },
  { from: "devices", to: "admin/devices", nested: false },
  { from: "settings/integrations", to: "admin/integrations", nested: true },
  { from: "settings/clinical", to: "admin/clinical", nested: false },
  { from: "settings/security", to: "account/security", nested: false },
];

/** Top-level segments that are not an org: a legacy redirect never applies under them. */
export const NON_ORG_SEGMENTS = ["api", "share", "login", "switch-org", "_next"] as const;
