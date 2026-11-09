import Link from "next/link";
import { notFound } from "next/navigation";
import { ClipsTable } from "@/components/ClipsTable";
import { FlagBadge } from "@/components/FlagBadge";
import { KpiCard } from "@/components/KpiCard";
import { QualityPanel } from "@/components/QualityPanel";
import { ReportPanel } from "@/components/ReportPanel";
import { notScoredLabel } from "@/components/ScoringStatus";
import { SessionTabs, parseTab } from "@/components/SessionTabs";
import { SessionReplay } from "@/components/replay/SessionReplay";
import { ReviewPanel } from "@/components/clinical/ReviewPanel";
import { ApiError } from "@/lib/api";
import { authedApi, currentRole } from "@/lib/auth/server";
import { can } from "@/lib/permissions";
import { problemText } from "@/lib/api/problem";
import { clinicalErrorMessage } from "@/messages/clinical";
import {
  amendReviewAction,
  claimReviewAction,
  cosignReviewAction,
  reloadReviewAction,
  saveDraftAction,
  signReviewAction,
} from "../../worklist/actions";
import { formatDateTime, formatNumber, formatPercent } from "@/lib/format";
import { loadOrNotFound } from "@/lib/loaders";
import { driftThresholds } from "@/lib/research/baselines";
import { enumLabel } from "@/messages/domain";
import { getText } from "@/lib/i18n/server";
import { pathsFor } from "@/lib/paths";
import { ageMonthsOfRecord, framesOfRecord } from "@/lib/asd/replay";

export default async function SessionDetailPage({
  params,
  searchParams,
}: {
  params: Promise<{ org: string; id: string }>;
  searchParams: Promise<{ tab?: string | string[] }>;
}) {
  const { org, id } = await params;
  const tab = parseTab((await searchParams).tab);
  const api = await authedApi();
  const session = await loadOrNotFound(() => api.getSession(id));
  // The id in the URL must belong to the org in the URL.
  if (session.org_id !== org) notFound();
  const text = await getText();
  const t = text.app.detail;
  const basePath = pathsFor(org).clinic.session(id);
  const role = await currentRole();
  const showClinical = can(role, "worklist.read");
  const clips = tab === "clips" ? await loadOrNotFound(() => api.getSessionClips(id)) : [];
  // INV-2: the Quality tab draws the drift warn/hard lines from the effective scoring profile. A role without config
  // access, or a profile that states none, shows no lines (never a hard-coded number).
  const drift = tab === "quality"
    ? driftThresholds((await api.getEffectiveConfig(org).catch(() => null))?.scoring_profile.body)
    : { warnDeg: null, hardDeg: null };
  return (
    <>
      <p>
        <Link href={pathsFor(org).clinic.sessions}>{t.back}</Link>
      </p>
      <h1>
        {session.participant_token} <span className="muted">{formatDateTime(session.started_at)}</span>
      </h1>
      <div className="kpi-grid">
        <KpiCard label={t.composite} value={notScoredLabel(session, text.app) ?? formatNumber(session.composite_score, 1, text.locale)} />
        <KpiCard label={t.confidence} value={formatPercent(session.composite_confidence, text.locale)} />
        <section className="kpi" aria-label={t.flag}>
          <div className="kpi-label">{t.flag}</div>
          <div className="kpi-value"><FlagBadge flag={session.attention_flag} /></div>
        </section>
        <KpiCard label={t.quality} value={enumLabel(text.locale, "quality_verdict", session.quality_verdict)} />
        <KpiCard label={t.bundle} value={session.bundle_version ?? "-"} />
      </div>
      <SessionTabs basePath={basePath} active={tab} hidden={showClinical ? [] : ["clinical"]} labels={t.tabs} label={t.tabsLabel} />
      {tab === "clips" ? <ClipsTable clips={clips} /> : null}
      {tab === "quality" ? <QualityPanel session={session} warnDeg={drift.warnDeg} hardDeg={drift.hardDeg} /> : null}
      {tab === "replay" ? <ReplayTab org={org} sessionId={id} /> : null}
      {tab === "report" ? <ReportPanel session={session} /> : null}
      {tab === "clinical" ? (
        showClinical ? (
          <ClinicalTab org={org} sessionId={id} role={role} />
        ) : (
          <p className="muted">{text.clinical.review.denied}</p>
        )
      ) : null}
    </>
  );
}

/**
 * WP-X.20 Replay tab: the stored record's frames (`GET /sessions/{id}/raw`, audited `session.raw_read`), the effective
 * catalog, baselines and profile (GOV-6), and the clips' stored scoring status. A role without config access gets no
 * catalog or baselines: the replay then draws events without areas or bands.
 */
async function ReplayTab({ org, sessionId }: { org: string; sessionId: string }) {
  const api = await authedApi();
  const [raw, effective, clips] = await Promise.all([
    // A role that may not read the stored record (403) or a purged one (404/410) replays nothing.
    api.getSessionRaw(sessionId).catch((err: unknown) => {
      if (err instanceof ApiError && [403, 404, 410].includes(err.status)) return null;
      throw err;
    }),
    api.getEffectiveConfig(org).catch(() => null),
    api.getSessionClips(sessionId).catch(() => []),
  ]);
  const record = raw?.raw_payload ?? null;
  return (
    <SessionReplay
      sessionId={sessionId}
      frames={framesOfRecord(record)}
      ageMonths={ageMonthsOfRecord(record)}
      catalog={effective?.stimulus_catalog.body ?? null}
      baselines={effective?.normative_baselines.body ?? null}
      profile={effective?.scoring_profile.body ?? null}
      clipStatuses={Object.fromEntries(clips.map((c) => [c.clip_id, c.status ?? null]))}
    />
  );
}

/** Loads the session's review (404 = not started) and wires the server actions. Clinical text never enters a URL. */
async function ClinicalTab({ org, sessionId, role }: { org: string; sessionId: string; role: Awaited<ReturnType<typeof currentRole>> }) {
  const api = await authedApi();
  const text = await getText();
  let review;
  try {
    review = await api.getSessionClinicalReview(sessionId);
  } catch (err) {
    if (err instanceof ApiError && err.status !== 401) {
      return <p role="alert" className="error">{err.problem ? problemText(text, err.problem).text : clinicalErrorMessage(err.slug, err.status)}</p>;
    }
    throw err;
  }
  return (
    <ReviewPanel
      // Keyed by id only: a server refresh after an action must not discard the panel's notices and dialogs.
      key={review?.id ?? "none"}
      review={review}
      canWrite={can(role, "clinical.write")}
      canSign={can(role, "clinical.sign")}
      canCosign={can(role, "clinical.cosign")}
      canContent={can(role, "clinical.content")}
      actions={{
        claim: claimReviewAction.bind(null, org, sessionId),
        save: saveDraftAction.bind(null, org),
        sign: signReviewAction.bind(null, org),
        cosign: cosignReviewAction.bind(null, org),
        amend: amendReviewAction.bind(null, org),
        reload: reloadReviewAction,
      }}
    />
  );
}
