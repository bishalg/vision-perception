"use client";

import { memo, useCallback, useEffect, useMemo, useRef } from "react";
import type { ParamRegistry, ParamSpec } from "@/lib/api/research";
import { useText } from "@/lib/i18n/provider";
import { diffParams, groupRegistry, setAt, valueOf, type ProfileDoc } from "@/lib/research/registry";
import type { ResearchMessages } from "@/messages/research";
import { ParamControl, type ParamContext } from "./ParamControl";
import r from "./research.module.css";

/** The problems reported for `id` or for an element of it (`ageBands[0].minMonths`). */
export function errorFor(errors: Record<string, string>, id: string): string | undefined {
  const found = Object.entries(errors)
    .filter(([k]) => k === id || k.startsWith(`${id}[`))
    .map(([, v]) => v);
  return found.length ? found.join("; ") : undefined;
}

/** One row; memoised so a keystroke in one knob does not re-render the other 87. */
const Row = memo(function Row({
  spec,
  value,
  published,
  context,
  error,
  text,
  onParam,
}: {
  spec: ParamSpec;
  value: unknown;
  published: unknown;
  context: ParamContext;
  error: string | undefined;
  text: ResearchMessages;
  onParam: (id: string, value: unknown) => void;
}) {
  return (
    <ParamControl spec={spec} value={value} published={published} context={context} error={error} text={text} onChange={(v) => onParam(spec.id, v)} />
  );
});

/**
 * The tuning form of a scoring profile (PRM-6, RUI-2), generated from the parameter registry: groups by first path
 * segment in registry order, one `ParamControl` per row chosen by its kind. There is no per-knob code; a new knob is a
 * registry row plus two message keys. `capture_only` rows are disabled in the `preview` context. `onChange` gets the
 * whole next draft document.
 */
export function RegistryForm({
  registry,
  published,
  draft,
  context,
  onChange,
  errors = {},
  text: textProp,
}: {
  registry: ParamRegistry;
  published: ProfileDoc;
  draft: ProfileDoc;
  context: ParamContext;
  onChange: (next: ProfileDoc) => void;
  errors?: Record<string, string>;
  text?: ResearchMessages;
}) {
  const bundle = useText();
  const text = textProp ?? bundle.research;
  const groups = useMemo(() => groupRegistry(registry.params), [registry]);
  const diff = diffParams(registry.params, published, draft);
  const latest = useRef({ draft, onChange });
  useEffect(() => {
    latest.current = { draft, onChange };
  });
  const onParam = useCallback((id: string, value: unknown) => latest.current.onChange(setAt(latest.current.draft, id, value)), []);
  return (
    <div className="stack">
      <p role="status" className={r.diffCount}>{text.form.diffCount(diff.length)}</p>
      {groups.map((g) => (
        <section key={g.group} className="panel stack" aria-labelledby={`group-${g.group}`} data-group={g.group}>
          <h3 id={`group-${g.group}`} style={{ margin: 0 }}>{text.groups[g.group] ?? g.group}</h3>
          {g.params.map((spec) => (
            <Row
              key={spec.id}
              spec={spec}
              value={valueOf(draft, spec)}
              published={valueOf(published, spec)}
              context={context}
              error={errorFor(errors, spec.id)}
              text={text}
              onParam={onParam}
            />
          ))}
        </section>
      ))}
    </div>
  );
}
