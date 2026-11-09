import type { Metadata } from "next";
import Link from "next/link";
import { Distribution } from "@/components/Distribution";
import { KpiCard } from "@/components/KpiCard";
import { TotpNudge } from "@/components/security/TotpNudge";
import { loadOrNotFound } from "@/lib/loaders";
import { authedApi } from "@/lib/auth/server";
import { getText } from "@/lib/i18n/server";
import { enumLabel, type DomainGroup } from "@/messages/domain";
import { pathsFor } from "@/lib/paths";

export async function generateMetadata(): Promise<Metadata> {
  return { title: (await getText()).app.overview.title };
}

export default async function OverviewPage({ params }: { params: Promise<{ org: string }> }) {
  const { org } = await params;
  const api = await authedApi();
  const [m, profile] = await Promise.all([
    loadOrNotFound(() => api.getMetrics(org)),
    // Only for the TOTP nudge: a failure hides the nudge rather than the overview.
    api.getMyProfile().catch(() => null),
  ]);
  const text = await getText();
  const t = text.app.overview;
  // Bar labels are enum values: show the catalogue label (an unknown value stays visible in ‹ ›).
  const labelled = (group: DomainGroup, counts: Record<string, number>) =>
    Object.fromEntries(Object.entries(counts).map(([k, n]) => [enumLabel(text.locale, group, k), n]));
  return (
    <>
      <h1>{t.title}</h1>
      <TotpNudge org={org} profile={profile} />
      <div className="kpi-grid">
        <KpiCard label={t.sessions7d} value={m.sessions_7d} />
        <KpiCard label={t.sessions30d} value={m.sessions_30d} />
        <KpiCard label={t.reviewQueue} value={m.review_queue_size} />
        <KpiCard label={t.devicesActive} value={m.devices_active} />
      </div>
      <p>
        <Link href={pathsFor(org).clinic.review}>{t.openQueue}</Link>
      </p>
      <div className="two-col">
        <Distribution title={t.flags} counts={labelled("attention_flag", m.flag_distribution)} />
        <Distribution title={t.quality} counts={labelled("quality_verdict", m.quality_distribution)} />
        <Distribution
          title={t.adoption}
          counts={Object.fromEntries(m.bundle_adoption.map((b) => [b.bundle_version, b.devices]))}
        />
      </div>
    </>
  );
}
