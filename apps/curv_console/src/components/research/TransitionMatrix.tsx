import { researchText, type ResearchMessages } from "@/messages/research";

export type Transition = readonly [string, string, number | null];

/**
 * A from/to table of state changes (published profile to draft). A cell the backend suppressed (PRV-1, `null`) shows the
 * "n < k" message and never `0`; a pair the run did not list shows a dash. Counts are drawn as sent.
 */
export function TransitionMatrix({
  caption,
  transitions,
  k,
  stateLabel,
  text = researchText,
}: {
  caption: string;
  transitions: readonly Transition[];
  k: number;
  /** Display name of a state (a flag, a quality status); the raw value when omitted. */
  stateLabel?: (state: string) => string;
  text?: ResearchMessages;
}) {
  const label = stateLabel ?? ((s: string) => s);
  const froms = [...new Set(transitions.map((x) => x[0]))];
  const tos = [...new Set(transitions.map((x) => x[1]))];
  const cell = new Map<string, number | null>(transitions.map((x) => [`${x[0]}\u0000${x[1]}`, x[2]]));
  return (
    <div className="table-wrap">
      <table className="data" aria-label={caption}>
        <caption className="muted">{caption}</caption>
        <thead>
          <tr>
            <th scope="col">{`${text.charts.matrixFrom} / ${text.charts.matrixTo}`}</th>
            {tos.map((to) => <th key={to} scope="col">{label(to)}</th>)}
          </tr>
        </thead>
        <tbody>
          {froms.map((from) => (
            <tr key={from}>
              <th scope="row">{label(from)}</th>
              {tos.map((to) => {
                const key = `${from}\u0000${to}`;
                const n = cell.get(key);
                return (
                  <td key={to} className="num" data-testid="transition-cell">
                    {n === undefined ? text.charts.noTransition : n === null ? text.suppressedCell(k) : n}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
