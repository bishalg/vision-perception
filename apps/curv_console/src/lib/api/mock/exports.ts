/**
 * Fixture-backed implementation of the exports and integrations API for mock mode (WP-I.8). Synthetic data only.
 * It mimics the backend's request-time checks that the UI surfaces (reason length, consent purpose, hl7v2 rules,
 * secret shape) so error paths can be seen in the demo; the real checks live in the Rust backend.
 */
import { ApiError, PROBLEM_BASE } from "../types";
import {
  type CreateIntegrationRequest,
  type Delivery,
  type Export,
  type ExportRequest,
  type ExportsApi,
  type Integration,
  type IntegrationConfig,
  type UpdateIntegrationRequest,
} from "../exports";

export interface MockExportsContext {
  orgId: string;
  userId: string;
  now: number;
  ensureOrg: (orgId: string) => void;
  page: <T>(
    rows: T[],
    key: (row: T) => [string, string],
    cursor: string | undefined,
    limit?: number,
  ) => { slice: T[]; next_cursor: string | null; total: number };
}

function problem(status: number, slug: string, title: string, detail?: string, params?: { name: string; reason: string }[]) {
  return new ApiError(status, {
    type: `${PROBLEM_BASE}${slug}`,
    title,
    status,
    detail,
    ...(params ? { invalid_params: params } : {}),
  });
}

const notFound = (what: string) => problem(404, "not-found", "Not found", `${what} not found`);

/** The seeded ready case package (WP-C.19): its share links are seeded by `mock/share.ts`. */
export const MOCK_CASE_EXPORT_ID = "e20c0de0-0000-4000-8000-000000000005";

/** What the mock share links (`mock/share.ts`) read and update of the exports held here. */
export interface MockExportStore {
  find(id: string): Export | undefined;
  update(id: string, patch: Partial<Export>): void;
}

/**
 * Hand-off to `createMockShare`: `createMockClient` (mock.ts, not owned by WP-C.19) builds the exports mock and then,
 * synchronously in the same call, the share mock without passing this store, so the share mock takes the store
 * built last. `MockShareContext.exports` takes precedence when mock.ts passes it explicitly.
 */
let latestStore: MockExportStore | null = null;
export function latestMockExportStore(): MockExportStore | null {
  return latestStore;
}

function mockUuid(kind: number, n: number): string {
  return `e${kind}0c0de0-0000-4000-8000-${String(n).padStart(12, "0")}`;
}

/** The mock secret check: `whsec_` + base64 of 24..64 bytes (the backend is the authority). */
function secretProblem(secret: string): string | null {
  if (!secret.startsWith("whsec_")) return "must start with whsec_";
  try {
    const n = atob(secret.slice(6)).length;
    return n >= 24 && n <= 64 ? null : "must hold 24..64 random bytes";
  } catch {
    return "must be base64";
  }
}

export function createMockExports(ctx: MockExportsContext): ExportsApi {
  const { orgId, userId, now } = ctx;
  const iso = (offsetMs: number) => new Date(now + offsetMs).toISOString();
  let seq = 100;

  const integrations = new Map<string, Integration>();
  const addIntegration = (i: Integration) => integrations.set(i.id, i);
  const webhookId = mockUuid(1, 1);
  const sftpId = mockUuid(1, 2);
  addIntegration({
    id: webhookId, org_id: orgId, kind: "webhook", name: "EHR webhook (test)", environment: "test", status: "active",
    config: { url: "https://ehr.example.org/hooks/curv", events: ["export.ready"], trigger: "manual", format: "csv" },
    has_secret: true, secret_fingerprint: "1a2b3c4d", ssh_public_key: null,
    last_test_at: iso(-3_600_000), last_test_ok: true, last_test_error: null,
    created_at: iso(-20 * 86_400_000), updated_at: iso(-3_600_000),
  });
  addIntegration({
    id: sftpId, org_id: orgId, kind: "sftp", name: "Hospital SFTP drop", environment: "production", status: "active",
    config: {
      host: "sftp.hospital.example.org", port: 22, username: "curv", remote_dir: "/inbound/curv", file_prefix: "curv",
      host_key_sha256: "SHA256:mockhostkeyfingerprintmockhostkeyfingerprint0", trigger: "on_final_sign",
      format: "fhir_bundle", mode: "identified",
    },
    has_secret: false, secret_fingerprint: null, ssh_public_key: null,
    last_test_at: iso(-86_400_000), last_test_ok: false, last_test_error: "connect-failed",
    created_at: iso(-10 * 86_400_000), updated_at: iso(-86_400_000),
  });

  const base = (n: number, over: Partial<Export>): Export => ({
    id: mockUuid(2, n), org_id: orgId, kind: "bulk", format: "csv", mode: "deidentified", purpose: "research",
    status: "succeeded", filters: {}, reason: "Quarterly research extract for the cohort study", destination_ids: [],
    created_at: iso(-n * 86_400_000), excluded: {}, requested_by: userId,
    byte_size: null, completed_at: null, content_type: null, error: null, expires_at: null, file_name: null,
    row_count: null, sha256: null, ...over,
  });
  const exportsList: Export[] = [
    base(1, { status: "queued" }),
    base(2, {
      status: "succeeded", format: "csv", filters: { from: "2026-10-01", flag: "red" }, destination_ids: [webhookId],
      row_count: 42, byte_size: 18204, content_type: "application/zip", file_name: "export-2-csv.zip",
      sha256: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
      completed_at: iso(-2 * 86_400_000 + 4000), expires_at: iso(86_400_000),
      excluded: { no_research_consent: 3, withdrawn: 1 },
    }),
    base(3, { status: "failed", format: "xlsx", error: "build-failed" }),
    base(4, {
      status: "expired", format: "csv_excel", row_count: 12, byte_size: 9000, file_name: "export-4-csv.zip",
      content_type: "application/zip", completed_at: iso(-4 * 86_400_000 + 4000), expires_at: iso(-86_400_000),
    }),
    base(5, {
      id: MOCK_CASE_EXPORT_ID, format: "case_package", mode: "identified", purpose: "clinical_care",
      reason: "Second opinion requested by the family", row_count: 3, byte_size: 48_211,
      content_type: "application/zip", file_name: "case-package-e20c0de0.zip",
      sha256: "2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae",
      completed_at: iso(-5 * 86_400_000 + 9000), expires_at: iso(6 * 86_400_000),
      case_participant_id: "c0c0c0c0-0000-4000-8000-000000000001", share_links_n: 4,
    }),
  ];
  const deliveries: Delivery[] = [
    {
      id: mockUuid(3, 1), export_id: exportsList[1]!.id, destination_id: webhookId, destination_name: "EHR webhook (test)",
      event: "export.ready", status: "delivered", attempts: 1, created_at: iso(-2 * 86_400_000),
      updated_at: iso(-2 * 86_400_000 + 6000), delivered_at: iso(-2 * 86_400_000 + 6000), last_error_class: null, remote_ref: "http 204",
    },
    {
      id: mockUuid(3, 2), export_id: exportsList[1]!.id, destination_id: sftpId, destination_name: "Hospital SFTP drop",
      event: "export.ready", status: "failed", attempts: 5, created_at: iso(-2 * 86_400_000),
      updated_at: iso(-2 * 86_400_000 + 600_000), delivered_at: null, last_error_class: "connect-failed", remote_ref: null,
    },
  ];

  const byKey = <T extends { created_at: string; id: string }>(rows: T[]) =>
    [...rows].sort((a, b) => b.created_at.localeCompare(a.created_at) || b.id.localeCompare(a.id));
  const key = <T extends { created_at: string; id: string }>(r: T): [string, string] => [r.created_at, r.id];
  const snap = <T,>(v: T): T => structuredClone(v);

  const exportById = (id: string) => {
    const e = exportsList.find((x) => x.id === id);
    if (!e) throw notFound("Export");
    return e;
  };
  const integrationById = (id: string) => {
    const i = integrations.get(id);
    if (!i) throw notFound("Integration");
    return i;
  };

  /** SHR-7 request rules, as the backend states them (the backend is the authority). */
  function caseParams(req: ExportRequest): { name: string; reason: string }[] {
    const params: { name: string; reason: string }[] = [];
    if (req.format !== "case_package") {
      if (req.case) params.push({ name: "case", reason: "only for case_package exports" });
      return params;
    }
    const c = req.case;
    if (!c) return [{ name: "case", reason: "is required for case_package exports" }];
    if (!c.participant_id) params.push({ name: "case.participant_id", reason: "is required" });
    const recipient = c.recipient.trim();
    if (recipient.length < 10 || recipient.length > 200) params.push({ name: "case.recipient", reason: "must be 10..200 characters" });
    if ((c.consent_reference ?? "").length > 200) params.push({ name: "case.consent_reference", reason: "must be at most 200 characters" });
    const filters = Object.values(req.filters ?? {}).filter((v) => v != null && !(Array.isArray(v) && v.length === 0));
    if (filters.length) params.push({ name: "filters", reason: "must be empty for case_package (the case names the participant)" });
    return params;
  }

  function validateExport(req: ExportRequest): void {
    const params: { name: string; reason: string }[] = caseParams(req);
    const reason = req.reason.trim();
    if (reason.length < 10 || reason.length > 500) params.push({ name: "reason", reason: "must be 10..500 characters" });
    if (req.format === "pdf_summary") params.push({ name: "format", reason: "pdf_summary is not available yet" });
    if (req.format === "hl7v2" && req.mode === "deidentified") {
      params.push({ name: "mode", reason: "hl7v2 exports are identified only (HL7-3)" });
    }
    if ((req.format === "dataset_jsonl" || req.format === "dataset_parquet") && req.mode === "identified") {
      params.push({ name: "mode", reason: "dataset exports are de-identified only (WP-B.9)" });
    }
    if ((req.format === "fhir_bundle" || req.format === "hl7v2") && req.filters?.session_ids?.length !== 1) {
      params.push({ name: "filters.session_ids", reason: `${req.format} holds exactly one session; name it here` });
    }
    for (const id of req.destination_ids ?? []) {
      const d = integrations.get(id);
      if (!d || d.status !== "active") params.push({ name: "destination_ids", reason: "unknown or disabled destination" });
    }
    if (params.length) throw problem(400, "validation-failed", "Validation failed", undefined, params);
    if (req.mode === "identified" && (req.purpose === "research" || req.purpose === "quality_improvement")) {
      throw problem(400, "consent-purpose-mismatch", "Consent purpose mismatch", "identified exports are not allowed for this purpose");
    }
    if (req.format === "case_package" && req.mode === "identified" && (!req.case?.consent_reference?.trim() || !req.case.consent_recorded_at)) {
      throw problem(400, "case-consent-missing", "Consent missing", "an identified case package needs the consent reference and date");
    }
  }

  function validateConfig(kind: Integration["kind"], c: IntegrationConfig): { name: string; reason: string }[] {
    const params: { name: string; reason: string }[] = [];
    if (kind === "webhook" && !c.url?.startsWith("https://")) params.push({ name: "config.url", reason: "must be an https:// URL" });
    if (kind === "sftp") {
      for (const f of ["host", "username", "remote_dir", "host_key_sha256"] as const) {
        if (!c[f]) params.push({ name: `config.${f}`, reason: "is required for sftp" });
      }
    }
    return params;
  }

  const trimName = (name: string, params: { name: string; reason: string }[]) => {
    const n = name.trim();
    if (n.length < 1 || n.length > 120) params.push({ name: "name", reason: "must be 1..120 characters" });
    return n;
  };

  latestStore = {
    find: (id) => exportsList.find((x) => x.id === id),
    update: (id, patch) => {
      const e = exportsList.find((x) => x.id === id);
      if (e) Object.assign(e, patch);
    },
  };

  return {
    async listExports(org, query) {
      ctx.ensureOrg(org);
      const { slice, next_cursor, total } = ctx.page(byKey(exportsList), key, query?.cursor, query?.limit ?? 20);
      return snap({ items: slice, next_cursor, total });
    },
    async getExport(id) {
      return snap(exportById(id));
    },
    async createExport(org, req) {
      ctx.ensureOrg(org);
      validateExport(req);
      seq += 1;
      const created: Export = base(0, {
        id: mockUuid(2, seq), created_at: iso(seq * 1000), status: "queued", format: req.format, mode: req.mode,
        purpose: req.purpose, reason: req.reason.trim(), filters: snap(req.filters ?? {}),
        destination_ids: [...new Set(req.destination_ids ?? [])].sort(),
        ...(req.format === "dataset_jsonl" || req.format === "dataset_parquet"
          ? { include_labels: req.include_labels ?? false }
          : {}),
      });
      if (req.format === "case_package" && req.case) {
        // Synthetic: the mock "builds" a case package at once so the share-link flow can be tried end to end.
        Object.assign(created, {
          status: "succeeded", case_participant_id: req.case.participant_id, share_links_n: 0,
          row_count: req.case.session_ids?.length || 3, byte_size: 40_960, content_type: "application/zip",
          file_name: `case-package-${created.id.slice(0, 8)}.zip`,
          sha256: "fcde2b2edba56bf408601fb721fe9b5c338d10ee429ea04fae5511b68fbf8fb9",
          completed_at: created.created_at, expires_at: iso(7 * 86_400_000),
        } satisfies Partial<Export>);
      }
      exportsList.push(created);
      for (const d of created.destination_ids) {
        const dest = integrations.get(d)!;
        seq += 1;
        deliveries.push({
          id: mockUuid(3, seq), export_id: created.id, destination_id: d, destination_name: dest.name,
          event: "export.ready", status: "pending", attempts: 0, created_at: created.created_at,
          updated_at: created.created_at, delivered_at: null, last_error_class: null, remote_ref: null,
        });
      }
      return snap(created);
    },
    async createExportDownloadUrl(id) {
      const e = exportById(id);
      if (e.status === "expired") throw problem(410, "export-expired", "Export expired", "the export artifact has expired; request a new export");
      if (e.status !== "succeeded") throw problem(409, "conflict", "Conflict", `the export is ${e.status}; nothing to download`);
      return { url: `/v1/exports/${e.id}/file?t=mock-token`, expires_at: iso(300_000) };
    },
    async listExportDeliveries(id, query) {
      exportById(id);
      const rows = byKey(deliveries.filter((d) => d.export_id === id));
      const { slice, next_cursor, total } = ctx.page(rows, key, query?.cursor, query?.limit ?? 20);
      return snap({ items: slice, next_cursor, total });
    },
    async listIntegrations(org, query) {
      ctx.ensureOrg(org);
      const { slice, next_cursor, total } = ctx.page(byKey([...integrations.values()]), key, query?.cursor, query?.limit);
      return snap({ items: slice, next_cursor, total });
    },
    async listExportDestinations(org) {
      ctx.ensureOrg(org);
      const items = [...integrations.values()]
        .filter((i) => i.status === "active" && (i.kind === "webhook" || i.kind === "sftp"))
        .sort((a, b) => a.created_at.localeCompare(b.created_at))
        .map(({ id, name, kind, environment, status }) => ({ id, name, kind, environment, status }));
      return { items };
    },
    async deleteIntegration(id) {
      integrationById(id);
      const inFlight = deliveries.filter((d) => d.destination_id === id && (d.status === "pending" || d.status === "sending")).length;
      if (inFlight > 0) {
        throw problem(409, "conflict", "Conflict", `${inFlight} deliveries to this destination are still pending`);
      }
      integrations.delete(id);
    },
    async getIntegration(id) {
      return snap(integrationById(id));
    },
    async createIntegration(org, req: CreateIntegrationRequest) {
      ctx.ensureOrg(org);
      const params: { name: string; reason: string }[] = [];
      const name = trimName(req.name, params);
      if (req.kind === "fhir_server" || req.kind === "hl7_mllp") params.push({ name: "kind", reason: `${req.kind} destinations are not available yet` });
      else params.push(...validateConfig(req.kind, req.config ?? {}));
      if (params.length) throw problem(400, "validation-failed", "Validation failed", undefined, params);
      seq += 1;
      const at = iso(seq * 1000);
      const created: Integration = {
        id: mockUuid(1, seq), org_id: orgId, kind: req.kind, name, environment: req.environment ?? "test", status: "active",
        config: snap(req.config ?? {}), has_secret: false, secret_fingerprint: null, ssh_public_key: null,
        last_test_at: null, last_test_ok: null, last_test_error: null, created_at: at, updated_at: at,
      };
      addIntegration(created);
      return snap(created);
    },
    async updateIntegration(id, patch: UpdateIntegrationRequest) {
      const cur = integrationById(id);
      const params: { name: string; reason: string }[] = [];
      const name = patch.name != null ? trimName(patch.name, params) : cur.name;
      if (patch.config) params.push(...validateConfig(cur.kind, patch.config));
      if (params.length) throw problem(400, "validation-failed", "Validation failed", undefined, params);
      const next: Integration = {
        ...cur, name, config: snap(patch.config ?? cur.config), environment: patch.environment ?? cur.environment,
        status: patch.status ?? cur.status, updated_at: iso(0),
      };
      integrations.set(id, next);
      return snap(next);
    },
    async putIntegrationSecret(id, secret) {
      const cur = integrationById(id);
      if (cur.kind !== "webhook") throw problem(409, "conflict", "Conflict", "only webhook destinations take a secret");
      const why = secretProblem(secret);
      if (why) throw problem(400, "validation-failed", "Validation failed", undefined, [{ name: "secret", reason: why }]);
      seq += 1;
      // A fake fingerprint, never derived from the secret.
      const next: Integration = { ...cur, has_secret: true, secret_fingerprint: seq.toString(16).padStart(8, "0"), updated_at: iso(0) };
      integrations.set(id, next);
      return snap(next);
    },
    async generateIntegrationSshKey(id) {
      const cur = integrationById(id);
      if (cur.kind !== "sftp") throw problem(409, "conflict", "Conflict", "only sftp destinations authenticate with an SSH key");
      seq += 1;
      const publicKey = `ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIMOCK${String(seq).padStart(10, "0")}KEYKEYKEYKEYKEYKEYKEYKEYKEY curv-asd-${id.slice(0, 8)}`;
      integrations.set(id, { ...cur, has_secret: true, ssh_public_key: publicKey, secret_fingerprint: seq.toString(16).padStart(8, "0"), updated_at: iso(0) });
      return { public_key: publicKey, fingerprint: `SHA256:mockfingerprint${String(seq).padStart(6, "0")}` };
    },
    async testIntegration(id) {
      const cur = integrationById(id);
      const ok = cur.has_secret && cur.status === "active";
      const result = {
        ok, tested_at: iso(0), error_class: ok ? null : "missing-secret",
        remote_ref: ok ? (cur.kind === "webhook" ? "http 204" : "/inbound/curv/curv-test-TEST-0000.json") : null,
      };
      integrations.set(id, { ...cur, last_test_at: result.tested_at, last_test_ok: ok, last_test_error: result.error_class });
      return result;
    },
    async listIntegrationDeliveries(id, query) {
      integrationById(id);
      const rows = byKey(deliveries.filter((d) => d.destination_id === id));
      const { slice, next_cursor, total } = ctx.page(rows, key, query?.cursor, query?.limit ?? 20);
      return snap({ items: slice, next_cursor, total });
    },
  };
}
