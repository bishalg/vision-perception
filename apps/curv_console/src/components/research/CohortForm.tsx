"use client";

import { useRouter } from "next/navigation";
import { useState, useTransition } from "react";
import type { CohortDefinition, CohortPreview as Preview } from "@/lib/api/research";
import { QUALITY_VERDICTS, TRACKING_VERDICTS } from "@/lib/api/types";
import { useText } from "@/lib/i18n/provider";
import { failureText, type ResearchResult } from "@/lib/research/result";
import type { ResearchMessages } from "@/messages/research";
import { CohortPreview } from "./CohortPreview";
import r from "./research.module.css";
import { pathsFor } from "@/lib/paths";

const toggle = (list: string[], v: string) => (list.includes(v) ? list.filter((x) => x !== v) : [...list, v]);

/**
 * New cohort (RUI-5, COH-1): the selection fields, a counts-only preview (suppressed cells show "n < k") and Save.
 * Only the fields the user filled in enter the definition, so an empty field selects everything.
 */
export function CohortForm({
  orgId,
  create,
  preview,
  text: textProp,
}: {
  orgId: string;
  create: (input: { name: string; definition: CohortDefinition }) => Promise<ResearchResult<{ id: string }>>;
  preview: (definition: CohortDefinition) => Promise<ResearchResult<{ preview: Preview }>>;
  text?: ResearchMessages;
}) {
  const bundle = useText();
  const text = textProp ?? bundle.research;
  const t = text.cohorts;
  const router = useRouter();
  const [name, setName] = useState("");
  const [bands, setBands] = useState("");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [quality, setQuality] = useState<string[]>([]);
  const [tracking, setTracking] = useState<string[]>([]);
  const [minValid, setMinValid] = useState("");
  const [frames, setFrames] = useState(false);
  const [synthetic, setSynthetic] = useState(false);
  const [counts, setCounts] = useState<Preview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, start] = useTransition();

  function definition(): CohortDefinition {
    const ageBands = bands.split(/[\s,]+/).filter(Boolean);
    const ratio = minValid.trim() === "" ? null : Number(minValid);
    return {
      ...(ageBands.length ? { age_bands: ageBands } : {}),
      ...(from ? { from } : {}),
      ...(to ? { to } : {}),
      ...(quality.length ? { quality_verdicts: quality } : {}),
      ...(tracking.length ? { tracking_verdicts: tracking } : {}),
      ...(ratio != null && Number.isFinite(ratio) ? { min_valid_ratio: ratio } : {}),
      ...(frames ? { require_frames: true } : {}),
      ...(synthetic ? { include_synthetic: true } : {}),
    };
  }

  function countPreview() {
    setError(null);
    start(async () => {
      const res = await preview(definition());
      if (res.ok) setCounts(res.preview);
      else setError(failureText(res, text));
    });
  }

  function save() {
    setError(null);
    if (!name.trim()) return setError(t.nameRequired);
    start(async () => {
      const res = await create({ name: name.trim(), definition: definition() });
      if (res.ok) router.push(pathsFor(orgId).research.cohort(res.id));
      else setError(failureText(res, text));
    });
  }

  return (
    <form className="panel stack" aria-label={t.newTitle} onSubmit={(ev) => { ev.preventDefault(); save(); }}>
      <label>
        {t.name}
        <input value={name} maxLength={200} onChange={(ev) => setName(ev.currentTarget.value)} />
      </label>
      <label>
        {t.ageBands}
        <input value={bands} onChange={(ev) => { setBands(ev.currentTarget.value); setCounts(null); }} />
      </label>
      <p className="muted" style={{ margin: 0 }}>{t.ageBandsHint}</p>
      <div className={r.row}>
        <label>{t.from}<input type="date" value={from} onChange={(ev) => { setFrom(ev.currentTarget.value); setCounts(null); }} /></label>
        <label>{t.to}<input type="date" value={to} onChange={(ev) => { setTo(ev.currentTarget.value); setCounts(null); }} /></label>
        <label>{t.minValidRatio}<input inputMode="decimal" value={minValid} onChange={(ev) => { setMinValid(ev.currentTarget.value); setCounts(null); }} /></label>
      </div>
      <fieldset>
        <legend>{t.qualityVerdicts}</legend>
        {QUALITY_VERDICTS.map((v) => (
          <label key={v} className="check">
            <input type="checkbox" checked={quality.includes(v)} onChange={() => { setQuality(toggle(quality, v)); setCounts(null); }} />
            {v}
          </label>
        ))}
      </fieldset>
      <fieldset>
        <legend>{t.trackingVerdicts}</legend>
        {TRACKING_VERDICTS.map((v) => (
          <label key={v} className="check">
            <input type="checkbox" checked={tracking.includes(v)} onChange={() => { setTracking(toggle(tracking, v)); setCounts(null); }} />
            {v}
          </label>
        ))}
      </fieldset>
      <label className="check">
        <input type="checkbox" checked={frames} onChange={(ev) => { setFrames(ev.currentTarget.checked); setCounts(null); }} />
        {t.requireFrames}
      </label>
      <label className="check">
        <input type="checkbox" checked={synthetic} onChange={(ev) => { setSynthetic(ev.currentTarget.checked); setCounts(null); }} />
        {t.includeSynthetic}
      </label>
      <div className={r.row}>
        <button type="button" className="secondary" disabled={pending} onClick={countPreview}>{pending ? t.previewing : t.preview}</button>
        <button type="submit" disabled={pending}>{pending ? t.saving : t.save}</button>
      </div>
      {counts ? <section aria-label={t.previewTitle} className="stack"><h2>{t.previewTitle}</h2><CohortPreview preview={counts} text={text} /></section> : null}
      {error ? <p role="alert" className="error">{error}</p> : null}
    </form>
  );
}
