"use client";

import { useState } from "react";
import type { AnalysisRun } from "@/lib/api/research";
import { useText } from "@/lib/i18n/provider";
import type { ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

/** RUN-5: every field that identifies what a run computed and with which code, in display order. */
export const PROVENANCE_FIELDS = [
  "definition_hash",
  "cohort_hash",
  "membership_hash",
  "profile_hash",
  "published_profile_version",
  "baselines_version",
  "catalog_version",
  "bundle_version",
  "core_version",
  "contract_version",
  "pipeline_version",
  "registry_version",
  "seed",
  "level_requested",
  "level_resolved",
] as const;
export type ProvenanceField = (typeof PROVENANCE_FIELDS)[number];
/** WP-X.11: shown only for a run started from a frozen dataset. */
export const DATASET_PROVENANCE_FIELDS = ["dataset_id", "dataset_hash"] as const;

function fieldValue(run: AnalysisRun, field: ProvenanceField, text: ResearchMessages): string | null {
  if (field === "level_resolved") {
    if (run.sessions_frames_n == null && run.sessions_score_n == null) return null;
    const none = text.runs.provenance.notRecorded;
    return `${text.level.frames}: ${run.sessions_frames_n ?? none}, ${text.level.score}: ${run.sessions_score_n ?? none}`;
  }
  if (field === "level_requested") return text.level[run.level_requested];
  const v = run[field];
  return v == null ? null : String(v);
}

function CopyButton({ label, value, text }: { label: string; value: string; text: ResearchMessages }) {
  const [copied, setCopied] = useState(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard access can be blocked; the value stays selectable on the page.
    }
  }
  return (
    <button type="button" className="secondary" aria-label={text.runs.provenance.copyField(label)} onClick={copy}>
      {copied ? text.runs.provenance.copied : text.runs.provenance.copy}
    </button>
  );
}

/** The RUN-5 provenance of a run: every field, copyable. A field the backend did not record says so, never a blank. */
export function ProvenanceBlock({ run, text: textProp }: { run: AnalysisRun; text?: ResearchMessages }) {
  const bundle = useText();
  const text = textProp ?? bundle.research;
  const t = text.runs.provenance;
  return (
    <section className="panel stack" aria-label={t.title}>
      <h2>{t.title}</h2>
      <p className="muted">{t.intro}</p>
      <dl className={r.provenance}>
        {DATASET_PROVENANCE_FIELDS.map((field) => {
          const value = run[field];
          if (value == null) return null;
          return (
            <div key={field} style={{ display: "contents" }} data-provenance={field}>
              <dt>{t.fields[field]}</dt>
              <dd>
                <code>{value}</code>
                <CopyButton label={t.fields[field]!} value={value} text={text} />
              </dd>
            </div>
          );
        })}
        {PROVENANCE_FIELDS.map((field) => {
          const value = fieldValue(run, field, text);
          return (
            <div key={field} style={{ display: "contents" }} data-provenance={field}>
              <dt>{t.fields[field]}</dt>
              <dd>
                {value == null ? (
                  <span className="muted">{t.notRecorded}</span>
                ) : (
                  <>
                    <code>{value}</code>
                    <CopyButton label={t.fields[field]!} value={value} text={text} />
                  </>
                )}
              </dd>
            </div>
          );
        })}
      </dl>
    </section>
  );
}
