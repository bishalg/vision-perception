import type { Metadata } from "next";
import s from "@/components/catalog/catalog.module.css";
import { SandboxRunner } from "@/components/config/SandboxRunner";
import type { ConfigDocument, ConfigDocumentKind } from "@/lib/api/catalog";
import { authedApi, currentRole } from "@/lib/auth/server";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { catalogText } from "@/messages/catalog";
import { configText } from "@/messages/config";
import { loadSandboxSessionsAction } from "../docActions";

const t = configText.sandbox;

export const metadata: Metadata = { title: t.title };

const first = (v: string | string[] | undefined) => (Array.isArray(v) ? v[0] : v)?.slice(0, 64);

/** The draft to compare by default: the newest draft or approved version, else the published one. */
function pickDraft(items: ConfigDocument[], wanted: string | undefined): ConfigDocument | undefined {
  if (wanted) return items.find((d) => d.version === wanted);
  return items.find((d) => d.status === "draft" || d.status === "candidate") ?? items.find((d) => d.status === "published");
}

/**
 * `/[org]/config/sandbox?profile=<version>&baselines=<version>`: re-score sessions of this org with a draft scoring
 * profile and baseline set against the published ones (spec 03 IA). Read-only for everyone who reads config.
 */
export default async function SandboxPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { org } = await params;
  const role = await currentRole();
  if (!can(role, "config.read")) {
    return (
      <>
        <h1>{t.title}</h1>
        <p className="muted">{configText.denied}</p>
      </>
    );
  }
  const raw = await searchParams;
  const api = await authedApi();
  const list = (kind: ConfigDocumentKind) =>
    loadOrNotFound(() => api.listConfigDocuments(kind, { limit: 100 })).then((p) => p.items.filter((d) => d.status !== "retired"));
  const [profiles, baselineSets, sessionPage] = await Promise.all([
    list("scoring_profile"),
    list("normative_baselines"),
    loadOrNotFound(() => api.listSessions(org, { limit: 50 })),
  ]);
  const publishedProfile = profiles.find((d) => d.status === "published");
  const publishedBaselines = baselineSets.find((d) => d.status === "published");
  const draftProfile = pickDraft(profiles, first(raw.profile));
  const draftBaselines = pickDraft(baselineSets, first(raw.baselines));
  const sessions = sessionPage.items;

  const label = (d: ConfigDocument | undefined, none: string) => (d ? `${d.version}` : none);
  // Record the run for the draft versions that are not the published ones (the publish gate).
  const record = [
    ...(draftProfile && draftProfile.status !== "published" ? [{ kind: "scoring_profile" as const, version: draftProfile.version }] : []),
    ...(draftBaselines && draftBaselines.status !== "published"
      ? [{ kind: "normative_baselines" as const, version: draftBaselines.version }]
      : []),
  ];
  const option = (d: ConfigDocument) => (
    <option key={d.id} value={d.version}>{d.version} ({catalogText.statuses[d.status]})</option>
  );

  return (
    <>
      <h1>{t.title}</h1>
      <p className="muted">{t.intro}</p>
      <form method="get" className={`panel ${s.row}`} aria-label={t.pick}>
        <label>
          {t.profile}
          <select name="profile" defaultValue={draftProfile?.version ?? ""}>
            {profiles.map(option)}
          </select>
        </label>
        <label>
          {t.baselines}
          <select name="baselines" defaultValue={draftBaselines?.version ?? ""}>
            {baselineSets.map(option)}
          </select>
        </label>
        <button type="submit" className="secondary">{t.apply}</button>
      </form>
      <p>
        {t.publishedLabel}: {label(publishedProfile, t.defaultProfile)} + {label(publishedBaselines, t.publishedNone)} · {t.draftLabel}:{" "}
        {label(draftProfile, t.defaultProfile)} + {label(draftBaselines, t.publishedNone)}
      </p>
      {draftBaselines ? (
        <SandboxRunner
          key={`${draftProfile?.version ?? ""}|${draftBaselines.version}`}
          sessions={sessions}
          initialSelected={sessions.slice(0, 10).map((x) => x.id)}
          published={
            publishedBaselines
              ? {
                  label: `${label(publishedProfile, t.defaultProfile)} + ${publishedBaselines.version}`,
                  profile: publishedProfile?.body ?? null,
                  baselines: publishedBaselines.body,
                }
              : null
          }
          draft={{
            label: `${label(draftProfile, t.defaultProfile)} + ${draftBaselines.version}`,
            profile: draftProfile?.body ?? null,
            baselines: draftBaselines.body,
          }}
          load={loadSandboxSessionsAction}
          record={record}
        />
      ) : (
        <p className="muted">{t.noBaselines}</p>
      )}
    </>
  );
}
