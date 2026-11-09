"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { useText } from "@/lib/i18n/provider";
import { pathsFor } from "@/lib/paths";
import type { ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

/**
 * "Start run from this dataset" (WP-X.11): pick an experiment, then its workbench opens with the run dialog set to this
 * dataset (`?dataset=`). The dialog there starts the run (`POST /analysis-runs` with `dataset_id`).
 */
export function DatasetRunStart({
  orgId,
  datasetId,
  experiments,
  text: textProp,
}: {
  orgId: string;
  datasetId: string;
  experiments: { id: string; name: string }[];
  text?: ResearchMessages;
}) {
  const bundle = useText();
  const t = (textProp ?? bundle.research).datasets;
  const router = useRouter();
  const [experimentId, setExperimentId] = useState("");
  if (experiments.length === 0) return <p className="muted">{t.noExperiments}</p>;
  return (
    <div className={r.row}>
      <label>
        {t.experiment}
        <select value={experimentId} onChange={(ev) => setExperimentId(ev.currentTarget.value)}>
          <option value="">{t.experimentPick}</option>
          {experiments.map((e) => <option key={e.id} value={e.id}>{e.name}</option>)}
        </select>
      </label>
      <button
        type="button"
        disabled={!experimentId}
        onClick={() => router.push(pathsFor(orgId).research.experimentRunOnDataset(experimentId, datasetId))}
      >
        {t.startRun}
      </button>
    </div>
  );
}
