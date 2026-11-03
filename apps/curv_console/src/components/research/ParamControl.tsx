"use client";

import { useId, useState } from "react";
import type { ParamKind, ParamSpec } from "@/lib/api/research";
import { clampParam, formatValue, isNumeric, sameValue, stepOf } from "@/lib/research/registry";
import { researchText, type ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

export type ParamContext = "preview" | "edit";

export interface ParamControlProps {
  spec: ParamSpec;
  value: unknown;
  /** The published profile's value of this knob; shown as a marker when it differs from `value`. */
  published: unknown;
  onChange: (value: unknown) => void;
  context: ParamContext;
  /** Problems the Rust core or the server reported for this knob. */
  error?: string;
  text?: ResearchMessages;
}

type Point = [number, number];
type Band = { id: string; minMonths: number; maxMonths: number };

const asNumber = (v: unknown): number => (typeof v === "number" ? v : Number.NaN);
const asPoints = (v: unknown): Point[] =>
  Array.isArray(v) ? v.map((p) => (Array.isArray(p) ? ([asNumber(p[0]), asNumber(p[1])] as Point) : ([Number.NaN, Number.NaN] as Point))) : [];
const asBands = (v: unknown): Band[] =>
  Array.isArray(v)
    ? v.map((b) => {
        const o = (typeof b === "object" && b !== null ? b : {}) as Record<string, unknown>;
        return { id: String(o.id ?? ""), minMonths: asNumber(o.minMonths), maxMonths: asNumber(o.maxMonths) };
      })
    : [];

/** A number field that keeps what is typed (so "0." survives) and reports finite values only. Never clamps: the core says what is wrong. */
function NumberInput({
  id,
  value,
  onValue,
  disabled,
  invalid,
  describedBy,
  min,
  max,
  step,
  label,
}: {
  id: string;
  value: number;
  onValue: (v: number) => void;
  disabled: boolean;
  invalid: boolean;
  describedBy?: string;
  min?: number;
  max?: number;
  step?: number | "any";
  label?: string;
}) {
  const [typed, setTyped] = useState<string | null>(null);
  return (
    <input
      id={id}
      type="number"
      aria-label={label}
      min={min}
      max={max}
      step={step ?? "any"}
      disabled={disabled}
      value={typed ?? (Number.isFinite(value) ? String(value) : "")}
      aria-invalid={invalid || undefined}
      aria-describedby={describedBy}
      onChange={(ev) => {
        setTyped(ev.currentTarget.value);
        const n = ev.currentTarget.valueAsNumber;
        if (Number.isFinite(n)) onValue(n);
      }}
      onBlur={() => setTyped(null)}
    />
  );
}

function PointListEditor({
  spec,
  kind,
  value,
  onChange,
  disabled,
  text,
}: {
  spec: ParamSpec;
  kind: Extract<ParamKind, { type: "point_list" }>;
  value: unknown;
  onChange: (v: unknown) => void;
  disabled: boolean;
  text: ResearchMessages;
}) {
  const label = text.params[spec.id]?.label ?? spec.id;
  const points = asPoints(value);
  const set = (next: Point[]) => onChange(next);
  return (
    <div className={r.pointList}>
      <span className="muted">{text.form.pointCount(points.length)}</span>
      {points.map((p, i) => (
        <div key={i} className={r.pointRow} role="group" aria-label={text.form.pointsOf(label, i + 1)}>
          {([0, 1] as const).map((axis) => (
            <label key={axis}>
              {axis === 0 ? text.form.x : text.form.y}
              <NumberInput
                id={`${spec.id}-${i}-${axis}`}
                value={p[axis]}
                disabled={disabled}
                invalid={false}
                onValue={(n) => set(points.map((q, j) => (j === i ? (axis === 0 ? [n, q[1]] : [q[0], n]) : q) as Point))}
              />
            </label>
          ))}
          <button
            type="button"
            className="secondary"
            disabled={disabled || points.length <= kind.minPoints}
            onClick={() => set(points.filter((_, j) => j !== i))}
          >
            {text.form.removePoint}
          </button>
        </div>
      ))}
      <button type="button" className="secondary" disabled={disabled || points.length >= kind.maxPoints} onClick={() => set([...points, [0, 0]])}>
        {text.form.addPoint}
      </button>
    </div>
  );
}

function AgeBandsEditor({
  spec,
  kind,
  value,
  onChange,
  disabled,
  text,
}: {
  spec: ParamSpec;
  kind: Extract<ParamKind, { type: "age_bands" }>;
  value: unknown;
  onChange: (v: unknown) => void;
  disabled: boolean;
  text: ResearchMessages;
}) {
  const bands = asBands(value);
  const set = (next: Band[]) => onChange(next);
  const patch = (i: number, p: Partial<Band>) => set(bands.map((b, j) => (j === i ? { ...b, ...p } : b)));
  return (
    <div className={r.pointList}>
      {bands.map((b, i) => (
        <div key={i} className={r.pointRow} role="group" aria-label={text.form.pointsOf(text.params[spec.id]?.label ?? spec.id, i + 1)}>
          <label>
            {text.form.bandId}
            <input value={b.id} disabled={disabled} onChange={(ev) => patch(i, { id: ev.currentTarget.value })} />
          </label>
          <label>
            {text.form.minMonths}
            <NumberInput id={`${spec.id}-${i}-min`} value={b.minMonths} disabled={disabled} invalid={false} min={kind.minMonths} max={kind.maxMonths} step={1} onValue={(n) => patch(i, { minMonths: n })} />
          </label>
          <label>
            {text.form.maxMonths}
            <NumberInput id={`${spec.id}-${i}-max`} value={b.maxMonths} disabled={disabled} invalid={false} min={kind.minMonths} max={kind.maxMonths} step={1} onValue={(n) => patch(i, { maxMonths: n })} />
          </label>
          <button type="button" className="secondary" disabled={disabled || bands.length <= kind.minBands} onClick={() => set(bands.filter((_, j) => j !== i))}>
            {text.form.removeBand}
          </button>
        </div>
      ))}
      <button
        type="button"
        className="secondary"
        disabled={disabled || bands.length >= kind.maxBands}
        onClick={() => {
          const last = bands[bands.length - 1];
          const from = last ? last.maxMonths + 1 : kind.minMonths;
          set([...bands, { id: `${from}-${from}`, minMonths: from, maxMonths: from }]);
        }}
      >
        {text.form.addBand}
      </button>
    </div>
  );
}

/**
 * One registry row as a control (PRM-6, RUI-2): the component is chosen by the row's `kind`, never by its id. Shows
 * the label and help from the messages catalogue (the id when a key is missing), the unit, the limits, the default
 * marker with a reset link, the published value when it differs, and the `reprocess` badge. A `capture_only` knob is
 * disabled in the preview context.
 */
export function ParamControl({ spec, value, published, onChange, context, error, text = researchText }: ParamControlProps) {
  const uid = useId();
  const inputId = `param-${spec.id}`;
  const helpId = `${uid}-help`;
  const errId = `${uid}-err`;
  const msg = text.params[spec.id];
  const label = msg?.label ?? spec.id;
  const disabled = context === "preview" && spec.reprocess === "capture_only";
  const kind = spec.kind;
  const unit = "unit" in kind ? text.units[kind.unit] : "";
  const isDefault = sameValue(value, spec.default);
  const differsFromPublished = !sameValue(value, published);
  const describedBy = [msg?.help ? helpId : null, error ? errId : null].filter(Boolean).join(" ") || undefined;
  const outside =
    isNumeric(kind) && typeof value === "number" && Number.isFinite(value) && (value < kind.min || value > kind.max);
  const shownError = error ?? (outside && isNumeric(kind) ? text.form.outside(text.form.range(formatValue(kind.min), formatValue(kind.max))) : undefined);

  let control: React.ReactNode;
  if (isNumeric(kind)) {
    const num = asNumber(value);
    const step = stepOf(kind);
    control = (
      <div className={r.numberPair}>
        <input
          type="range"
          aria-label={text.form.slider(label)}
          min={kind.min}
          max={kind.max}
          step={step}
          disabled={disabled}
          value={Number.isFinite(num) ? Math.min(kind.max, Math.max(kind.min, num)) : kind.min}
          onChange={(ev) => {
            const v = clampParam(kind, ev.currentTarget.valueAsNumber);
            if (v !== null) onChange(v);
          }}
        />
        <NumberInput
          id={inputId}
          value={num}
          min={kind.min}
          max={kind.max}
          step={step}
          disabled={disabled}
          invalid={Boolean(shownError)}
          describedBy={describedBy}
          onValue={onChange}
        />
        {unit ? <span className="muted">{unit}</span> : null}
      </div>
    );
  } else if (kind.type === "bool") {
    control = (
      <input
        id={inputId}
        type="checkbox"
        checked={value === true}
        disabled={disabled}
        aria-describedby={describedBy}
        onChange={(ev) => onChange(ev.currentTarget.checked)}
      />
    );
  } else if (kind.type === "enum") {
    control = (
      <select id={inputId} value={String(value)} disabled={disabled} aria-describedby={describedBy} onChange={(ev) => onChange(ev.currentTarget.value)}>
        {kind.values.map((v) => (
          <option key={v} value={v}>{v}</option>
        ))}
      </select>
    );
  } else if (kind.type === "point_list") {
    control = <PointListEditor spec={spec} kind={kind} value={value} onChange={onChange} disabled={disabled} text={text} />;
  } else {
    control = <AgeBandsEditor spec={spec} kind={kind} value={value} onChange={onChange} disabled={disabled} text={text} />;
  }

  const composite = kind.type === "point_list" || kind.type === "age_bands";
  return (
    <div className={`${r.param} ${shownError ? r.paramInvalid : ""}`} data-param-id={spec.id} data-reprocess={spec.reprocess}>
      <div className={r.paramHead}>
        {composite ? <span className={r.paramLabel}>{label}</span> : <label htmlFor={inputId} className={r.paramLabel}>{label}</label>}
        <span className={`badge ${r.badge} ${r[`badge_${spec.reprocess}`] ?? ""}`}>{text.reprocess[spec.reprocess]}</span>
        {spec.ownerGate ? <span className={r.gate}>{text.ownerGated}</span> : null}
      </div>
      {control}
      {msg?.help ? <span id={helpId} className="muted">{msg.help}</span> : null}
      <div className={r.paramMeta}>
        {isNumeric(kind) ? <span className="muted">{text.form.range(formatValue(kind.min), formatValue(kind.max))}</span> : null}
        {kind.type === "point_list" || kind.type === "age_bands" ? null : <span className="muted">{text.form.defaultValue(formatValue(spec.default))}</span>}
        {differsFromPublished ? <span className="muted">{text.form.publishedValue(formatValue(published))}</span> : null}
        {!isDefault ? (
          <button type="button" className="link-button" disabled={disabled} onClick={() => onChange(spec.default)}>{text.form.reset}</button>
        ) : null}
        {disabled ? <span className="muted">{text.form.captureOnlyPreview}</span> : null}
      </div>
      {shownError ? <span id={errId} className={r.error} role="alert">{shownError}</span> : null}
    </div>
  );
}
