import type { Metadata } from "next";
import Link from "next/link";
import { Distribution } from "@/components/Distribution";
import { KpiCard } from "@/components/KpiCard";
import { loadOrNotFound } from "@/lib/loaders";
import { authedApi } from "@/lib/auth/server";
import { en } from "@/messages/en";

export const metadata: Metadata = { title: en.overview.title };

export default async function OverviewPage({ params }: { params: Promise<{ org: string }> }) {
  const { org } = await params;
  const api = await authedApi();
  const m = await loadOrNotFound(() => api.getMetrics(org));
  const t = en.overview;
  return (
    <>
      <h1>{t.title}</h1>
      <div className="kpi-grid">
        <KpiCard label={t.sessions7d} value={m.sessions_7d} />
        <KpiCard label={t.sessions30d} value={m.sessions_30d} />
        <KpiCard label={t.reviewQueue} value={m.review_queue_size} />
        <KpiCard label={t.devicesActive} value={m.devices_active} />
      </div>
      <p>
        <Link href={`/${encodeURIComponent(org)}/review`}>{t.openQueue}</Link>
      </p>
      <div className="two-col">
        <Distribution title={t.flags} counts={m.flag_distribution} />
        <Distribution title={t.quality} counts={m.quality_distribution} />
        <Distribution
          title={t.adoption}
          counts={Object.fromEntries(m.bundle_adoption.map((b) => [b.bundle_version, b.devices]))}
        />
      </div>
    </>
  );
}
