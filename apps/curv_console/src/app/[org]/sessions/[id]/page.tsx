import Link from "next/link";
import { notFound } from "next/navigation";
import { ClipsTable } from "@/components/ClipsTable";
import { FlagBadge } from "@/components/FlagBadge";
import { KpiCard } from "@/components/KpiCard";
import { QualityPanel } from "@/components/QualityPanel";
import { ReportPanel } from "@/components/ReportPanel";
import { SessionTabs, parseTab } from "@/components/SessionTabs";
import { ReviewPanel } from "@/components/clinical/ReviewPanel";
import { ApiError } from "@/lib/api";
import { authedApi, currentRole } from "@/lib/auth/server";
import { can } from "@/lib/permissions";
import { clinicalErrorMessage, clinicalText } from "@/messages/clinical";
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
import { en } from "@/messages/en";

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
  const t = en.detail;
  const basePath = `/${encodeURIComponent(org)}/sessions/${encodeURIComponent(id)}`;
  const role = await currentRole();
  const showClinical = can(role, "worklist.read");
  const clips = tab === "clips" ? await loadOrNotFound(() => api.getSessionClips(id)) : [];
  return (
    <>
      <p>
        <Link href={`/${encodeURIComponent(org)}/sessions`}>{t.back}</Link>
      </p>
      <h1>
        {session.participant_token} <span className="muted">{formatDateTime(session.started_at)}</span>
      </h1>
      <div className="kpi-grid">
        <KpiCard label={t.composite} value={formatNumber(session.composite_score)} />
        <KpiCard label={t.confidence} value={formatPercent(session.composite_confidence)} />
        <section className="kpi" aria-label={t.flag}>
          <div className="kpi-label">{t.flag}</div>
          <div className="kpi-value"><FlagBadge flag={session.attention_flag} /></div>
        </section>
        <KpiCard label={t.quality} value={session.quality_verdict} />
        <KpiCard label={t.bundle} value={session.bundle_version ?? "-"} />
      </div>
      <SessionTabs basePath={basePath} active={tab} hidden={showClinical ? [] : ["clinical"]} />
      {tab === "clips" ? <ClipsTable clips={clips} /> : null}
      {tab === "quality" ? <QualityPanel session={session} /> : null}
      {tab === "report" ? <ReportPanel session={session} /> : null}
      {tab === "clinical" ? (
        showClinical ? (
          <ClinicalTab org={org} sessionId={id} role={role} />
        ) : (
          <p className="muted">{clinicalText.review.denied}</p>
        )
      ) : null}
    </>
  );
}

/** Loads the session's review (404 = not started) and wires the server actions. Clinical text never enters a URL. */
async function ClinicalTab({ org, sessionId, role }: { org: string; sessionId: string; role: Awaited<ReturnType<typeof currentRole>> }) {
  const api = await authedApi();
  let review;
  try {
    review = await api.getSessionClinicalReview(sessionId);
  } catch (err) {
    if (err instanceof ApiError && err.status !== 401) {
      return <p role="alert" className="error">{clinicalErrorMessage(err.slug, err.status)}</p>;
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
