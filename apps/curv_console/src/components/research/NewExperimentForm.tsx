"use client";

import { useRouter } from "next/navigation";
import { useState, useTransition } from "react";
import { useText } from "@/lib/i18n/provider";
import { failureText, type ResearchResult } from "@/lib/research/result";
import type { ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

/** Name, description and the starting point of a new experiment; on success opens its workbench. */
export function NewExperimentForm({
  orgId,
  experiments,
  create,
  text: textProp,
}: {
  orgId: string;
  experiments: { id: string; name: string }[];
  create: (input: { name: string; description?: string; from_experiment_id?: string }) => Promise<ResearchResult<{ id: string }>>;
  text?: ResearchMessages;
}) {
  const bundle = useText();
  const text = textProp ?? bundle.research;
  const t = text.experiments;
  const router = useRouter();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [from, setFrom] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, start] = useTransition();

  function submit() {
    if (!name.trim()) {
      setError(t.nameRequired);
      return;
    }
    setError(null);
    start(async () => {
      const res = await create({
        name: name.trim(),
        ...(description.trim() ? { description: description.trim() } : {}),
        ...(from ? { from_experiment_id: from } : {}),
      });
      if (res.ok) router.push(`/${encodeURIComponent(orgId)}/research/experiments/${encodeURIComponent(res.id)}`);
      else setError(failureText(res, text));
    });
  }

  return (
    <form
      className="panel stack"
      onSubmit={(ev) => {
        ev.preventDefault();
        submit();
      }}
    >
      <label>
        {t.name}
        <input value={name} maxLength={200} onChange={(ev) => setName(ev.currentTarget.value)} aria-invalid={Boolean(error) || undefined} />
      </label>
      <label>
        {t.description}
        <textarea rows={2} value={description} maxLength={2000} onChange={(ev) => setDescription(ev.currentTarget.value)} />
      </label>
      <label>
        {t.startFrom}
        <select value={from} onChange={(ev) => setFrom(ev.currentTarget.value)}>
          <option value="">{t.startPublished}</option>
          {experiments.map((e) => <option key={e.id} value={e.id}>{e.name}</option>)}
        </select>
      </label>
      {error ? <p role="alert" className="muted">{error}</p> : null}
      <div className={r.row}>
        <button type="submit" disabled={pending}>{pending ? t.creating : t.create}</button>
      </div>
    </form>
  );
}
