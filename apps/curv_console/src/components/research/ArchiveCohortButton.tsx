"use client";

import { useRouter } from "next/navigation";
import { useState, useTransition } from "react";
import { failureText, type ResearchResult } from "@/lib/research/result";
import { researchText, type ResearchMessages } from "@/messages/research";

/** Archives a cohort (it stays readable; runs that used it keep their membership hash). */
export function ArchiveCohortButton({
  archive,
  text = researchText,
}: {
  archive: () => Promise<ResearchResult>;
  text?: ResearchMessages;
}) {
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
        {pending ? text.cohorts.archiving : text.cohorts.archive}
      </button>
      {error ? <p role="alert" className="error">{error}</p> : null}
    </div>
  );
}
