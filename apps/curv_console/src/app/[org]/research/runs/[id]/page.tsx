import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { AgeProfileTable } from "@/components/research/AgeProfileTable";
import { ProvenanceBlock } from "@/components/research/ProvenanceBlock";
import { RunResults } from "@/components/research/RunResults";
import { RunStatus } from "@/components/research/RunStatus";
import { SweepPanel } from "@/components/research/SweepPanel";
import { authedApi, currentRole } from "@/lib/auth/server";
import { formatDateTime } from "@/lib/format";
import { getText } from "@/lib/i18n/server";
import { loadOrNotFound } from "@/lib/loaders";
import { can } from "@/lib/permissions";
import { parseAgeProfile } from "@/lib/research/age-profile";
import { snapshotOf } from "@/lib/research/runs";
import { cancelRunAction, pollRunAction, rerunRunAction } from "../actions";


export async function generateMetadata(): Promise<Metadata> {
  const tx = await getText();
  const t = tx.research.runs;
  return { title: t.title };
}

/**
 * `/[org]/research/runs/[id]`: status with live polling, RUN-5 provenance, the sweep (SWP-3) or age-profile (AGE-1)
 * section of a phase-B run, and the RUN-4 results (RUI-6).
 */
export default async function RunPage({ params }: { params: Promise<{ org: string; id: string }> }) {
  const tx = await getText();
  const t = tx.research.runs;
  const { org, id } = await params;
  const role = await currentRole();
  const api = await authedApi();
  const run = await loadOrNotFound(() => api.getAnalysisRun(id));
  if (run.org_id !== org) notFound();
  const base = `/${encodeURIComponent(org)}/research`;
  const k = run.summary?.small_cell_k ?? 0;
  const ageProfile = parseAgeProfile(run.summary?.age_profile);
  const [experiment, cohort, effective] = await Promise.all([
    run.experiment_id ? api.getExperiment(run.experiment_id).catch(() => null) : null,
    run.cohort_id ? api.getCohort(run.cohort_id).catch(() => null) : null,
    // The published baseline ranges shade the histograms; without config access nothing is shaded.
    run.summary ? api.getEffectiveConfig(org).catch(() => null) : null,
  ]);
  return (
    <>
      <p><Link href={`${base}/runs`}>{t.allRuns}</Link></p>
      <h1>{t.heading(run.id)}</h1>
      <p className="muted">
        {t.kind[run.kind]}
        {experiment ? <> · <Link href={`${base}/experiments/${encodeURIComponent(experiment.id)}`}>{experiment.name}</Link></> : null}
        {cohort ? <> · <Link href={`${base}/cohorts/${encodeURIComponent(cohort.id)}`}>{cohort.name}</Link></> : null}
        {" · "}{formatDateTime(run.created_at)}
      </p>
      <RunStatus
        key={run.id}
        orgId={org}
        runId={run.id}
        initial={snapshotOf(run)}
        poll={pollRunAction.bind(null, org)}
        cancel={cancelRunAction.bind(null, org)}
        rerun={rerunRunAction.bind(null, org)}
        canRun={can(role, "analysis.run")}
        canExport={can(role, "export.create")}
      />
      {run.status === "succeeded" ? (
        <p><Link href={`${base}/runs/${encodeURIComponent(run.id)}/sessions`}>{t.sessionsLink}</Link></p>
      ) : null}
      <ProvenanceBlock run={run} />
      {run.summary?.sweep ? <SweepPanel sweep={run.summary.sweep} k={k} text={tx.research} locale={tx.locale} /> : null}
      {ageProfile ? (
        <AgeProfileTable profile={ageProfile} k={k} text={tx.research} />
      ) : run.summary?.age_profile != null ? (
        <p className="muted">{tx.research.analysis.unreadable}</p>
      ) : null}
      {run.summary ? (
        <RunResults text={tx.research} summary={run.summary} baselines={effective?.normative_baselines.body ?? null} membershipDrift={run.membership_drift} />
      ) : run.status === "succeeded" ? (
        <p className="muted">{t.results.noSummary}</p>
      ) : null}
    </>
  );
}
