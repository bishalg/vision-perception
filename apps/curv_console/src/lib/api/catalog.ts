/**
 * Config documents and the stimulus catalog (WP-C.4; backend WP-B.4 `routes/config.rs`): zod schemas for the
 * `ConfigDocument*` DTOs and the `StimulusCatalog` body of `docs/spec/schemas/asd/api-v1.json`, the `CatalogApi`
 * interface and its live HTTP implementation.
 *
 * The schemas check shape only (types and required members, as the contract does). Every rule beyond shape is the
 * Rust backend's: it validates the body when a draft is created and again on approve, and answers problem+json.
 */
import { z } from "@/lib/zod";
import type { LiveRequest } from "./clinical";

const str = z.string().nullable().optional();

export const CONFIG_DOCUMENT_KINDS = ["stimulus_catalog", "normative_baselines", "scoring_profile"] as const;
export const configDocumentKindSchema = z.enum(CONFIG_DOCUMENT_KINDS);
export type ConfigDocumentKind = z.infer<typeof configDocumentKindSchema>;

/** Lifecycle `draft -> candidate -> published -> retired`; only `published` goes into a bundle. */
export const CONFIG_DOCUMENT_STATUSES = ["draft", "candidate", "published", "retired"] as const;
export const configDocumentStatusSchema = z.enum(CONFIG_DOCUMENT_STATUSES);
export type ConfigDocumentStatus = z.infer<typeof configDocumentStatusSchema>;

export const configDocumentSchema = z.object({
  id: z.string(),
  kind: configDocumentKindSchema,
  version: z.string(),
  status: configDocumentStatusSchema,
  /** `StimulusCatalog`, `NormativeBaselineSet` or `ScoringProfile`: parse with the kind's schema before use. */
  body: z.custom<unknown>((v) => v !== undefined),
  /** null for a system-seeded document. */
  created_by: str,
  created_at: z.string(),
  published_at: str,
  notes: str,
});
export type ConfigDocument = z.infer<typeof configDocumentSchema>;

export const configDocumentPageSchema = z.object({
  items: z.array(configDocumentSchema),
  next_cursor: z.string().nullable().optional(),
  total: z.number().int().min(0).nullable().optional(),
});
export type ConfigDocumentPage = z.infer<typeof configDocumentPageSchema>;

export const createConfigDocumentRequestSchema = z.object({
  version: z.string(),
  body: z.custom<unknown>((v) => v !== undefined),
  notes: str,
});
export type CreateConfigDocumentRequest = z.infer<typeof createConfigDocumentRequestSchema>;

// ---- StimulusCatalog body (asd-core `types::catalog`) -----------------------------------------------------------

export const agePackSchema = z.object({ id: z.string(), minMonths: z.number().int(), maxMonths: z.number().int() });
export type AgePack = z.infer<typeof agePackSchema>;

/** Battery membership: protocol id (e.g. `express_30s`) and play order. */
export const membershipSchema = z.object({ id: z.string(), order: z.number().int() });
export type Membership = z.infer<typeof membershipSchema>;

/** An AOI box: time range in clip seconds, coordinates normalised to the frame (0..1, origin top left). */
export const areaSchema = z.object({
  name: z.string(),
  startSec: z.number(),
  endSec: z.number(),
  xMin: z.number(),
  yMin: z.number(),
  xMax: z.number(),
  yMax: z.number(),
  roles: z.array(z.string()),
});
export type Area = z.infer<typeof areaSchema>;

export const catalogClipSchema = z.object({
  id: z.string(),
  title: z.string(),
  category: z.string(),
  durationSec: z.number(),
  minMonths: z.number().int(),
  maxMonths: z.number().int(),
  protocols: z.array(membershipSchema),
  areas: z.array(areaSchema),
});
export type CatalogClip = z.infer<typeof catalogClipSchema>;

export const stimulusCatalogSchema = z.object({
  schemaVersion: z.string(),
  comment: str,
  languages: z.array(z.string()),
  fallbackLanguage: z.string(),
  agePacks: z.array(agePackSchema),
  clips: z.array(catalogClipSchema),
});
export type StimulusCatalog = z.infer<typeof stimulusCatalogSchema>;

/**
 * The AOI roles the catalog authoring guide names. Roles are free strings in the contract; the editor offers these
 * and keeps any other value it finds untouched.
 */
export const AOI_ROLES = ["social", "geometric", "eye", "mouth", "orienting"] as const;
export type AoiRole = (typeof AOI_ROLES)[number];

export interface ConfigListQuery {
  status?: ConfigDocumentStatus;
  cursor?: string;
  /** 1..100 (backend cap). */
  limit?: number;
}

export type ConfigTransition = "approve" | "publish" | "retire";

export interface CatalogApi {
  /** Newest first. Read by owners, admins, clinicians, researchers and the superadmin. */
  listConfigDocuments(kind: ConfigDocumentKind, query?: ConfigListQuery): Promise<ConfigDocumentPage>;
  getConfigDocument(kind: ConfigDocumentKind, version: string): Promise<ConfigDocument>;
  /** Superadmin only (global documents). Creates a `draft`; drafts are immutable, a change is a new version. */
  createConfigDocument(kind: ConfigDocumentKind, request: CreateConfigDocumentRequest): Promise<ConfigDocument>;
  /** Superadmin only, four-eyes rule (not the author; a seeded document is published by someone else than its approver). */
  transitionConfigDocument(kind: ConfigDocumentKind, version: string, step: ConfigTransition): Promise<ConfigDocument>;
}

export function createLiveCatalog(request: LiveRequest, buildQuery: (q: object) => string): CatalogApi {
  const enc = encodeURIComponent;
  return {
    listConfigDocuments: (kind, q) => request(`/config/${enc(kind)}${buildQuery(q ?? {})}`, configDocumentPageSchema),
    getConfigDocument: (kind, version) => request(`/config/${enc(kind)}/${enc(version)}`, configDocumentSchema),
    createConfigDocument: (kind, body) => request(`/config/${enc(kind)}`, configDocumentSchema, { method: "POST", body }),
    transitionConfigDocument: (kind, version, step) =>
      request(`/config/${enc(kind)}/${enc(version)}/${step}`, configDocumentSchema, { method: "POST" }),
  };
}
