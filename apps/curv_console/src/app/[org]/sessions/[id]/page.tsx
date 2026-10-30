import Link from "next/link";
import { notFound } from "next/navigation";
import { ClipsTable } from "@/components/ClipsTable";
import { FlagBadge } from "@/components/FlagBadge";
import { KpiCard } from "@/components/KpiCard";
import { QualityPanel } from "@/components/QualityPanel";
import { ReportPanel } from "@/components/ReportPanel";
import { SessionTabs, parseTab } from "@/components/SessionTabs";
import { authedApi } from "@/lib/auth/server";
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
  const clips = tab === "clips" ? await loadOrNotFound(() => api.getSessionClips(id)) : [];
  return (
    <>
      <p>
        <Link href={`/${encodeURIComponent(org)}/sessions`}>{t.back}</Link>
      </p>
      <h1>
        {session.participant_id} <span className="muted">{formatDateTime(session.started_at)}</span>
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
      <SessionTabs basePath={basePath} active={tab} />
      {tab === "clips" ? <ClipsTable clips={clips} /> : null}
      {tab === "quality" ? <QualityPanel session={session} /> : null}
      {tab === "report" ? <ReportPanel session={session} /> : null}
    </>
  );
}
