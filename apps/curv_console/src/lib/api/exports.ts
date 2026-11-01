/**
 * Exports and integration destinations (WP-I.8; backend WP-I.6/I.7): zod schemas for the DTOs of
 * `docs/spec/schemas/asd/api-v1.json`, the `ExportsApi` interface and its live HTTP implementation. The console
 * builds no export and holds no secret: it sends the request, shows what the API returns, and never stores a
 * secret or a signed download URL (the URL is resolved server-side and streamed or redirected, see
 * `app/api/export/[id]/route.ts`).
 */
import { z } from "zod";
import type { LiveRequest } from "./clinical";

const str = z.string().nullable().optional();
const count = z.number().int().min(0).nullable().optional();

export const EXPORT_FORMATS = [
  "csv", "csv_excel", "csv_sheets", "xlsx", "pdf_summary", "fhir_bundle", "fhir_ndjson", "hl7v2",
] as const;
export const exportFormatSchema = z.enum(EXPORT_FORMATS);
export type ExportFormat = z.infer<typeof exportFormatSchema>;

export const EXPORT_KINDS = ["bulk", "document"] as const;
export const exportKindSchema = z.enum(EXPORT_KINDS);

export const EXPORT_MODES = ["identified", "deidentified"] as const;
export const exportModeSchema = z.enum(EXPORT_MODES);
export type ExportMode = z.infer<typeof exportModeSchema>;

export const EXPORT_PURPOSES = ["clinical_care", "research", "quality_improvement", "patient_request"] as const;
export const exportPurposeSchema = z.enum(EXPORT_PURPOSES);
export type ExportPurpose = z.infer<typeof exportPurposeSchema>;

export const EXPORT_STATUSES = ["queued", "running", "succeeded", "failed", "expired"] as const;
export const exportStatusSchema = z.enum(EXPORT_STATUSES);
export type ExportStatus = z.infer<typeof exportStatusSchema>;

export const DELIVERY_STATUSES = ["pending", "sending", "delivered", "failed"] as const;
export const deliveryStatusSchema = z.enum(DELIVERY_STATUSES);
export type DeliveryStatus = z.infer<typeof deliveryStatusSchema>;

export const DELIVERY_EVENTS = ["export.ready", "review.finalized"] as const;
export const deliveryEventSchema = z.enum(DELIVERY_EVENTS);
export type DeliveryEvent = z.infer<typeof deliveryEventSchema>;

export const DELIVERY_TRIGGERS = ["manual", "on_final_sign"] as const;
export const deliveryTriggerSchema = z.enum(DELIVERY_TRIGGERS);
export type DeliveryTrigger = z.infer<typeof deliveryTriggerSchema>;

export const INTEGRATION_KINDS = ["webhook", "sftp", "fhir_server", "hl7_mllp"] as const;
export const integrationKindSchema = z.enum(INTEGRATION_KINDS);
export type IntegrationKind = z.infer<typeof integrationKindSchema>;

export const INTEGRATION_ENVIRONMENTS = ["test", "production"] as const;
export const integrationEnvironmentSchema = z.enum(INTEGRATION_ENVIRONMENTS);
export type IntegrationEnvironment = z.infer<typeof integrationEnvironmentSchema>;

export const INTEGRATION_STATUSES = ["active", "disabled"] as const;
export const integrationStatusSchema = z.enum(INTEGRATION_STATUSES);
export type IntegrationStatus = z.infer<typeof integrationStatusSchema>;

/** EXJ-1 filters. Every field optional; `clinical_state` takes review states or `none`. */
export const exportFiltersSchema = z.object({
  bundle_version: str,
  clinical_state: z.array(z.string()).optional(),
  flag: str,
  from: str,
  participant_ids: z.array(z.string()).optional(),
  quality: str,
  reviewed: z.boolean().nullable().optional(),
  session_ids: z.array(z.string()).optional(),
  to: str,
});
export type ExportFilters = z.infer<typeof exportFiltersSchema>;

/** Never carries clinical content: counts, hashes and the request. */
export const exportSchema = z.object({
  id: z.string(),
  org_id: z.string(),
  kind: exportKindSchema,
  format: exportFormatSchema,
  mode: exportModeSchema,
  purpose: exportPurposeSchema,
  status: exportStatusSchema,
  filters: exportFiltersSchema,
  reason: z.string(),
  destination_ids: z.array(z.string()),
  created_at: z.string(),
  /** Excluded sessions by reason (`withdrawn`, `no_research_consent`, `mode_not_allowed`, `capture_source_refused`). */
  excluded: z.record(z.string(), z.number().int().min(0)),
  byte_size: count,
  completed_at: str,
  content_type: str,
  error: str,
  expires_at: str,
  file_name: str,
  requested_by: str,
  row_count: count,
  sha256: str,
});
export type Export = z.infer<typeof exportSchema>;

export const exportPageSchema = z.object({
  items: z.array(exportSchema),
  next_cursor: z.string().nullable().optional(),
  total: count,
});
export type ExportPage = z.infer<typeof exportPageSchema>;

export const exportRequestSchema = z.object({
  format: exportFormatSchema,
  mode: exportModeSchema,
  purpose: exportPurposeSchema,
  /** 10..500 characters, shown in the audit log. */
  reason: z.string(),
  destination_ids: z.array(z.string()).optional(),
  filters: exportFiltersSchema.optional(),
});
export type ExportRequest = z.infer<typeof exportRequestSchema>;

export const downloadUrlSchema = z.object({ url: z.string(), expires_at: z.string() });
export type DownloadUrl = z.infer<typeof downloadUrlSchema>;

/** INTG-3 destination settings (one shape for every kind). `fhir` and `hl7` are opaque and not editable here. */
export const integrationConfigSchema = z.object({
  events: z.array(deliveryEventSchema).optional(),
  fhir: z.unknown().optional(),
  file_prefix: str,
  format: exportFormatSchema.nullable().optional(),
  hl7: z.unknown().optional(),
  host: str,
  host_key_sha256: str,
  mode: exportModeSchema.nullable().optional(),
  port: z.number().int().min(0).max(65535).nullable().optional(),
  remote_dir: str,
  trigger: deliveryTriggerSchema.nullable().optional(),
  url: str,
  username: str,
});
export type IntegrationConfig = z.infer<typeof integrationConfigSchema>;

/** Secrets never appear: `has_secret`, a short fingerprint and (sftp) the public key. */
export const integrationSchema = z.object({
  id: z.string(),
  org_id: z.string(),
  kind: integrationKindSchema,
  name: z.string(),
  environment: integrationEnvironmentSchema,
  status: integrationStatusSchema,
  config: integrationConfigSchema,
  has_secret: z.boolean(),
  created_at: z.string(),
  updated_at: z.string(),
  last_test_at: str,
  last_test_error: str,
  last_test_ok: z.boolean().nullable().optional(),
  secret_fingerprint: str,
  ssh_public_key: str,
});
export type Integration = z.infer<typeof integrationSchema>;

export const integrationPageSchema = z.object({
  items: z.array(integrationSchema),
  next_cursor: z.string().nullable().optional(),
  total: count,
});
export type IntegrationPage = z.infer<typeof integrationPageSchema>;

export const createIntegrationRequestSchema = z.object({
  kind: integrationKindSchema,
  name: z.string(),
  config: integrationConfigSchema.optional(),
  environment: integrationEnvironmentSchema.nullable().optional(),
});
export type CreateIntegrationRequest = z.infer<typeof createIntegrationRequestSchema>;

/** `config` replaces the whole config. */
export const updateIntegrationRequestSchema = z.object({
  config: integrationConfigSchema.nullable().optional(),
  environment: integrationEnvironmentSchema.nullable().optional(),
  name: z.string().nullable().optional(),
  status: integrationStatusSchema.nullable().optional(),
});
export type UpdateIntegrationRequest = z.infer<typeof updateIntegrationRequestSchema>;

/** Write-only: `whsec_` + base64 of 24..64 random bytes. Never returned. */
export const putIntegrationSecretRequestSchema = z.object({ secret: z.string() });
export type PutIntegrationSecretRequest = z.infer<typeof putIntegrationSecretRequestSchema>;

export const sshPublicKeySchema = z.object({ public_key: z.string(), fingerprint: z.string() });
export type SshPublicKey = z.infer<typeof sshPublicKeySchema>;

/** The outcome carries an error class only, never the remote response. */
export const integrationTestResultSchema = z.object({
  ok: z.boolean(),
  tested_at: z.string(),
  error_class: str,
  remote_ref: str,
});
export type IntegrationTestResult = z.infer<typeof integrationTestResultSchema>;

export const deliverySchema = z.object({
  id: z.string(),
  export_id: z.string(),
  destination_id: z.string(),
  destination_name: z.string(),
  event: deliveryEventSchema,
  status: deliveryStatusSchema,
  attempts: z.number().int().min(0),
  created_at: z.string(),
  updated_at: z.string(),
  delivered_at: str,
  last_error_class: str,
  remote_ref: str,
});
export type Delivery = z.infer<typeof deliverySchema>;

export const deliveryPageSchema = z.object({
  items: z.array(deliverySchema),
  next_cursor: z.string().nullable().optional(),
  total: count,
});
export type DeliveryPage = z.infer<typeof deliveryPageSchema>;

export interface ExportsPageQuery {
  cursor?: string;
  /** 1..100 (backend cap). */
  limit?: number;
}

export interface ExportsApi {
  listExports(orgId: string, query?: ExportsPageQuery): Promise<ExportPage>;
  getExport(exportId: string): Promise<Export>;
  createExport(orgId: string, request: ExportRequest): Promise<Export>;
  /** Signed URL valid five minutes; absolute (presigned) or relative `/v1/exports/{id}/file?t=` (proxy mode). */
  createExportDownloadUrl(exportId: string): Promise<DownloadUrl>;
  listExportDeliveries(exportId: string, query?: ExportsPageQuery): Promise<DeliveryPage>;
  listIntegrations(orgId: string): Promise<IntegrationPage>;
  getIntegration(integrationId: string): Promise<Integration>;
  createIntegration(orgId: string, request: CreateIntegrationRequest): Promise<Integration>;
  updateIntegration(integrationId: string, patch: UpdateIntegrationRequest): Promise<Integration>;
  /** Webhook only. Write-only: the response never contains the secret. */
  putIntegrationSecret(integrationId: string, secret: string): Promise<Integration>;
  /** SFTP only. Generates or rotates the key; the private half never leaves the backend. */
  generateIntegrationSshKey(integrationId: string): Promise<SshPublicKey>;
  testIntegration(integrationId: string): Promise<IntegrationTestResult>;
  listIntegrationDeliveries(integrationId: string, query?: ExportsPageQuery): Promise<DeliveryPage>;
}

export function createLiveExports(request: LiveRequest, buildQuery: (q: object) => string): ExportsApi {
  const enc = encodeURIComponent;
  return {
    listExports: (orgId, q) => request(`/orgs/${enc(orgId)}/exports${buildQuery(q ?? {})}`, exportPageSchema),
    getExport: (id) => request(`/exports/${enc(id)}`, exportSchema),
    createExport: (orgId, body) => request(`/orgs/${enc(orgId)}/exports`, exportSchema, { method: "POST", body }),
    createExportDownloadUrl: (id) => request(`/exports/${enc(id)}/download-url`, downloadUrlSchema, { method: "POST" }),
    listExportDeliveries: (id, q) =>
      request(`/exports/${enc(id)}/deliveries${buildQuery(q ?? {})}`, deliveryPageSchema),
    listIntegrations: (orgId) => request(`/orgs/${enc(orgId)}/integrations`, integrationPageSchema),
    getIntegration: (id) => request(`/integrations/${enc(id)}`, integrationSchema),
    createIntegration: (orgId, body) =>
      request(`/orgs/${enc(orgId)}/integrations`, integrationSchema, { method: "POST", body }),
    updateIntegration: (id, body) => request(`/integrations/${enc(id)}`, integrationSchema, { method: "PATCH", body }),
    putIntegrationSecret: (id, secret) =>
      request(`/integrations/${enc(id)}/secret`, integrationSchema, { method: "PUT", body: { secret } }),
    generateIntegrationSshKey: (id) =>
      request(`/integrations/${enc(id)}/ssh-key`, sshPublicKeySchema, { method: "POST" }),
    testIntegration: (id) => request(`/integrations/${enc(id)}/test`, integrationTestResultSchema, { method: "POST" }),
    listIntegrationDeliveries: (id, q) =>
      request(`/integrations/${enc(id)}/deliveries${buildQuery(q ?? {})}`, deliveryPageSchema),
  };
}
