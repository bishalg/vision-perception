import "server-only";

/**
 * Server-side loading shared by the baselines and scoring-profile "new draft" pages (WP-C.5): the source document
 * (`?from=` or the published one) and what the in-editor preview needs (the published documents and recent
 * sessions).
 */
import type { ConfigDocument, ConfigDocumentKind } from "@/lib/api/catalog";
import type { NormativeBaselineSet } from "@/lib/api/config";
import type { SessionSummary } from "@/lib/api";
import { authedApi } from "@/lib/auth/server";
import { parseBaselines } from "@/lib/configDocs";
import { loadOrNotFound } from "@/lib/loaders";

export interface DraftContext {
  source: ConfigDocument | undefined;
  preview: {
    sessions: SessionSummary[];
    published: { label: string; profile: unknown; baselines: NormativeBaselineSet } | null;
  };
}

const PREVIEW_SESSIONS = 10;

export async function loadDraftContext(
  org: string,
  kind: ConfigDocumentKind,
  from: string | undefined,
): Promise<DraftContext> {
  const api = await authedApi();
  const published = (k: ConfigDocumentKind) =>
    loadOrNotFound(() => api.listConfigDocuments(k, { status: "published", limit: 1 })).then((p) => p.items[0]);
  const [source, profile, baselines, sessions] = await Promise.all([
    from ? loadOrNotFound(() => api.getConfigDocument(kind, from)) : published(kind),
    published("scoring_profile"),
    published("normative_baselines"),
    // The preview is optional: a role that cannot list sessions still gets the editor.
    api.listSessions(org, { limit: PREVIEW_SESSIONS }).then((p) => p.items).catch(() => [] as SessionSummary[]),
  ]);
  const set = baselines ? parseBaselines(baselines.body) : null;
  return {
    source,
    preview: {
      sessions,
      published: set
        ? { label: `${profile?.version ?? "default"} + ${baselines!.version}`, profile: profile?.body ?? null, baselines: set }
        : null,
    },
  };
}
