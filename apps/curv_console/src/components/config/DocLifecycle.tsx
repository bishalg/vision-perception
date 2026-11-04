"use client";

import Link from "next/link";
import { useSyncExternalStore } from "react";
import { LifecycleActions } from "@/components/catalog/LifecycleActions";
import type { CatalogResult } from "@/components/catalog/types";
import type { ConfigDocumentKind, ConfigDocumentStatus, ConfigTransition } from "@/lib/api/catalog";
import { readSandboxRun } from "@/lib/configDocs";
import { formatDateTime } from "@/lib/format";
import { useText } from "@/lib/i18n/provider";


function subscribe(onChange: () => void): () => void {
  window.addEventListener("storage", onChange);
  return () => window.removeEventListener("storage", onChange);
}

/**
 * Lifecycle buttons for a baselines or scoring-profile version, with the console's publish gate (spec 03 §4.5):
 * publishing asks for a sandbox run of this version in this browser first. Approval by a second superadmin and the
 * changelog note are enforced by the backend and the draft form. The gate is the console's: the backend does not
 * know about sandbox runs.
 */
export function DocLifecycle({
  kind,
  version,
  status,
  hints,
  sandboxHref,
  run,
}: {
  kind: ConfigDocumentKind;
  version: string;
  status: ConfigDocumentStatus;
  hints: Record<ConfigDocumentStatus, string>;
  sandboxHref: string;
  run: (step: ConfigTransition) => Promise<CatalogResult<{ status: string }>>;
}) {
  const tx = useText();
  const g = tx.config.lifecycleGate;
  const raw = useSyncExternalStore(
    subscribe,
    () => {
      const r = readSandboxRun(kind, version);
      return r ? JSON.stringify(r) : "";
    },
    () => "",
  );
  const record = raw ? (JSON.parse(raw) as { at: string; sessions: number; changed: number }) : null;
  return (
    <LifecycleActions
      status={status}
      run={run}
      hints={hints}
      blocked={record ? undefined : { publish: g.publishNeedsSandbox }}
    >
      <p style={{ margin: 0 }}>
        {record ? g.lastRun(formatDateTime(record.at), record.sessions, record.changed) : g.noRun}{" "}
        <Link href={sandboxHref}>{tx.config.runSandbox}</Link>
      </p>
    </LifecycleActions>
  );
}
