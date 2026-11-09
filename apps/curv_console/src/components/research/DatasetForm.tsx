"use client";

import { useRouter } from "next/navigation";
import { useState, useTransition } from "react";
import type { CreateDatasetRequest, ExclusionRule } from "@/lib/api/research";
import { useText } from "@/lib/i18n/provider";
import { pathsFor } from "@/lib/paths";
import { failureText, type ResearchResult } from "@/lib/research/result";
import type { ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

/** The rule ids the form writes (stable, so two freezes with the same choices hash alike). */
const QUALITY_RULE = "quality";
const MANUAL_RULE = "manual";

/**
 * New dataset (WP-X.11, R-7): a cohort, a name, the exclusion rules (quality gates, named sessions with a reason),
 * clinician labels on or off, and an optional seeded split. Only what the user filled in enters the request; the
 * backend checks every value again, freezes the members and computes the hash.
 */
export function DatasetForm({
  orgId,
  cohorts,
  create,
  text: textProp,
}: {
  orgId: string;
  cohorts: { id: string; name: string }[];
  create: (input: CreateDatasetRequest) => Promise<ResearchResult<{ id: string }>>;
  text?: ResearchMessages;
}) {
  const bundle = useText();
  const text = textProp ?? bundle.research;
  const t = text.datasets.form;
  const router = useRouter();
  const [cohortId, setCohortId] = useState("");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [minValid, setMinValid] = useState("");
  const [poorTracking, setPoorTracking] = useState(false);
  const [sessions, setSessions] = useState("");
  const [reason, setReason] = useState("");
  const [labels, setLabels] = useState(false);
  const [split, setSplit] = useState(false);
  const [seed, setSeed] = useState("");
  const [fractions, setFractions] = useState({ train: "0.7", validation: "0.15", holdout: "0.15" });
  const [error, setError] = useState<string | null>(null);
  const [pending, start] = useTransition();

  function request(): CreateDatasetRequest {
    const rules: ExclusionRule[] = [];
    const ratio = minValid.trim() === "" ? null : Number(minValid);
    if ((ratio != null && Number.isFinite(ratio)) || poorTracking) {
      rules.push({
        kind: "quality_gates",
        id: QUALITY_RULE,
        ...(ratio != null && Number.isFinite(ratio) ? { min_valid_ratio: ratio } : {}),
        ...(poorTracking ? { exclude_tracking_verdicts: ["poor"] } : {}),
      });
    }
    const named = sessions.split(/[\s,]+/).map((x) => x.trim()).filter(Boolean);
    if (named.length) {
      rules.push({ kind: "explicit_list", id: MANUAL_RULE, sessions: [...new Set(named)].map((session_id) => ({ session_id, reason: reason.trim() })) });
    }
    const parsedSeed = seed.trim() === "" ? undefined : Number(seed);
    return {
      name: name.trim(),
      cohort_id: cohortId,
      ...(description.trim() ? { description: description.trim() } : {}),
      ...(rules.length ? { exclusion_rules: rules } : {}),
      ...(labels ? { labels: "review_outcomes" as const } : {}),
      ...(split
        ? {
            split: {
              ...(parsedSeed !== undefined && Number.isSafeInteger(parsedSeed) ? { seed: parsedSeed } : {}),
              fractions: { train: Number(fractions.train), validation: Number(fractions.validation), holdout: Number(fractions.holdout) },
            },
          }
        : {}),
    };
  }

  function save() {
    setError(null);
    if (!name.trim() || !cohortId) return setError(t.nameRequired);
    start(async () => {
      const res = await create(request());
      if (res.ok) {
        router.push(pathsFor(orgId).research.dataset(res.id));
        return;
      }
      if (res.slug === "conflict") return setError(t.nameTaken);
      if (res.params?.some((p) => p.name === "split.fractions")) return setError(t.fractionsInvalid);
      setError(failureText(res, text));
    });
  }

  if (cohorts.length === 0) return <p className="muted">{t.noCohorts}</p>;
  return (
    <form className="panel stack" aria-label={text.datasets.newTitle} onSubmit={(ev) => { ev.preventDefault(); save(); }}>
      <div className={r.row}>
        <label>
          {t.cohort}
          <select value={cohortId} onChange={(ev) => setCohortId(ev.currentTarget.value)}>
            <option value="">{t.cohortPick}</option>
            {cohorts.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
          </select>
        </label>
        <label>
          {t.name}
          <input value={name} maxLength={80} onChange={(ev) => setName(ev.currentTarget.value)} />
        </label>
      </div>
      <label>
        {t.description}
        <textarea value={description} maxLength={500} rows={2} onChange={(ev) => setDescription(ev.currentTarget.value)} />
      </label>
      <fieldset className="stack">
        <legend>{t.rulesTitle}</legend>
        <p className="muted" style={{ margin: 0 }}>{t.rulesIntro}</p>
        <label>
          {t.minValidRatio}
          <input type="number" min={0} max={1} step={0.05} className={r.ageInput} value={minValid} onChange={(ev) => setMinValid(ev.currentTarget.value)} />
        </label>
        <label>
          <input type="checkbox" checked={poorTracking} onChange={(ev) => setPoorTracking(ev.currentTarget.checked)} /> {t.excludePoorTracking}
        </label>
        <label>
          {t.excludeSessions}
          <textarea value={sessions} rows={3} onChange={(ev) => setSessions(ev.currentTarget.value)} aria-describedby="dataset-exclude-hint" />
        </label>
        <p id="dataset-exclude-hint" className="muted" style={{ margin: 0 }}>{t.excludeSessionsHint}</p>
        <label>
          {t.excludeReason}
          <input value={reason} maxLength={200} onChange={(ev) => setReason(ev.currentTarget.value)} />
        </label>
      </fieldset>
      <label>
        <input type="checkbox" checked={labels} onChange={(ev) => setLabels(ev.currentTarget.checked)} /> {t.labels}
      </label>
      <p className="muted" style={{ margin: 0 }}>{t.labelsHint}</p>
      <label>
        <input type="checkbox" checked={split} onChange={(ev) => setSplit(ev.currentTarget.checked)} /> {t.split}
      </label>
      {split ? (
        <fieldset className="stack">
          <legend>{t.split}</legend>
          <div className={r.row}>
            <label>
              {t.seed}
              <input inputMode="numeric" value={seed} onChange={(ev) => setSeed(ev.currentTarget.value)} />
            </label>
            {(["train", "validation", "holdout"] as const).map((k) => (
              <label key={k}>
                {text.datasets.splitCols[k]}
                <input
                  type="number"
                  min={0}
                  max={1}
                  step={0.05}
                  className={r.ageInput}
                  value={fractions[k]}
                  onChange={(ev) => {
                    const v = ev.currentTarget.value;
                    setFractions((cur) => ({ ...cur, [k]: v }));
                  }}
                />
              </label>
            ))}
          </div>
          <p className="muted" style={{ margin: 0 }}>{t.seedHint} {t.fractionsHint}</p>
        </fieldset>
      ) : null}
      {error ? <p role="alert" className={r.error}>{error}</p> : null}
      <div className={r.row}>
        <button type="submit" disabled={pending}>{pending ? t.saving : t.save}</button>
      </div>
    </form>
  );
}
