import type { CohortPreview as Preview } from "@/lib/api/research";
import type { ResearchMessages } from "@/messages/research";

/**
 * COH-2 preview: counts only. A count the backend suppressed (PRV-1, below `small_cell_k`) is `null` and shows the
 * "n < k" message, never `0`.
 */
export function CohortPreview({ preview, text }: { preview: Preview; text: ResearchMessages }) {
  const t = text.run;
  const k = preview.small_cell_k;
  const count = (n: number | null | undefined) => (n == null ? text.suppressedCell(k) : String(n));
  return (
    <div role="status" className="stack" data-testid="cohort-preview">
      <p style={{ margin: 0 }}>{t.previewCounts(count(preview.n), preview.by_level.frames, preview.by_level.score)}</p>
      <ul>
        {preview.by_age_band.map((b) => (
          <li key={b.band}>{t.bandRow(b.band, b.suppressed ? text.suppressedCell(k) : count(b.n))}</li>
        ))}
      </ul>
      <p className="muted" style={{ margin: 0 }}>
        {t.excluded(preview.excluded.no_consent, preview.excluded.withdrawn, preview.excluded.unscorable, preview.excluded.no_age, preview.excluded.quality)}
      </p>
    </div>
  );
}
