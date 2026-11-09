import type { Dataset } from "@/lib/api/research";
import { formatShare, splitRows, UNBANDED } from "@/lib/research/datasets";
import type { ResearchMessages } from "@/messages/research";

/**
 * The split of a frozen dataset (R-7): seed, shares, and members per age band and part as the backend counted them,
 * with a total row. A dataset without a split says so.
 */
export function SplitSummary({ dataset, text }: { dataset: Pick<Dataset, "split" | "split_counts">; text: ResearchMessages }) {
  const t = text.datasets;
  const split = dataset.split;
  if (!split) return <p className="muted">{t.noSplit}</p>;
  const rows = splitRows(dataset);
  const c = t.splitCols;
  return (
    <div className="stack">
      <p style={{ margin: 0 }}>{t.splitSeed(String(split.seed))}</p>
      <p className="muted" style={{ margin: 0 }}>
        {t.splitFractions(formatShare(split.fractions.train), formatShare(split.fractions.validation), formatShare(split.fractions.holdout))}
      </p>
      <div className="table-wrap">
        <table className="data" aria-label={t.splitTitle}>
          <thead>
            <tr>
              <th scope="col">{c.stratum}</th>
              <th scope="col">{c.train}</th>
              <th scope="col">{c.validation}</th>
              <th scope="col">{c.holdout}</th>
              <th scope="col">{c.total}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.total ? "\u0000total" : r.stratum} data-split-row={r.total ? "total" : r.stratum}>
                <th scope="row">{r.total ? t.total : r.stratum === UNBANDED ? t.unbanded : r.stratum}</th>
                <td>{r.train}</td>
                <td>{r.validation}</td>
                <td>{r.holdout}</td>
                <td>{r.all}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
