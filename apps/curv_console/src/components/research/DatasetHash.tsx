"use client";

import { useState } from "react";
import { useText } from "@/lib/i18n/provider";
import { shortHash } from "@/lib/research/datasets";
import type { ResearchMessages } from "@/messages/research";

/**
 * A dataset's content hash (R-7): the short form at a glance, the full hash one click away, and a copy button. The
 * hash is the backend's (`asd_contract::datasets::content_hash`); it is shown, never computed here.
 */
export function DatasetHash({ hash, text: textProp }: { hash: string; text?: ResearchMessages }) {
  const bundle = useText();
  const t = (textProp ?? bundle.research).datasets.hash;
  const [copied, setCopied] = useState(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(hash);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard access can be blocked; the full hash stays selectable below.
    }
  }
  return (
    <span data-testid="dataset-hash" data-hash={hash}>
      <code title={hash}>{shortHash(hash)}</code>{" "}
      <button type="button" className="secondary" aria-label={t.copy} onClick={copy}>
        {copied ? t.copied : t.copy}
      </button>
      <details>
        <summary>{t.full}</summary>
        <code style={{ overflowWrap: "anywhere" }}>{hash}</code>
      </details>
    </span>
  );
}
