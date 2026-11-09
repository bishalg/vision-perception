"use client";

import { useRouter } from "next/navigation";
import { useState, useTransition } from "react";
import { useText } from "@/lib/i18n/provider";
import { failureText, type ResearchResult } from "@/lib/research/result";
import type { ResearchMessages } from "@/messages/research";

/** Archives a dataset: it stays readable (members, hash, lineage) but starts no new run. */
export function ArchiveDatasetButton({ archive, text: textProp }: { archive: () => Promise<ResearchResult>; text?: ResearchMessages }) {
  const bundle = useText();
  const text = textProp ?? bundle.research;
  const router = useRouter();
  const [error, setError] = useState<string | null>(null);
  const [pending, start] = useTransition();
  return (
    <div className="stack">
      <button
        type="button"
        className="secondary"
        disabled={pending}
        onClick={() =>
          start(async () => {
            const res = await archive();
            if (res.ok) router.refresh();
            else setError(failureText(res, text));
          })
        }
      >
        {pending ? text.datasets.archiving : text.datasets.archive}
      </button>
      {error ? <p role="alert" className="error">{error}</p> : null}
    </div>
  );
}
