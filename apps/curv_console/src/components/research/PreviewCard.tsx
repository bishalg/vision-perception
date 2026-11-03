import type { ScoreSide, Trace } from "@/lib/research/preview";
import { formatNumber } from "@/lib/format";
import { researchText, type ResearchMessages } from "@/messages/research";
import { ScanpathPlot, type PlotRect } from "./ScanpathPlot";
import r from "./research.module.css";

export interface CardData {
  id: string;
  token: string;
  framesAvailable: boolean;
  published: ScoreSide | null;
  draft: ScoreSide | null;
  /** `param_not_applied` ids at the level shown. */
  notApplied: string[];
  /** Why frame-level re-processing was not done (shown once per card). */
  framesNote: boolean;
  publishedTrace: Trace | null;
  draftTrace: Trace | null;
  aois: PlotRect[];
  error: string | null;
}

function Side({ title, side, text }: { title: string; side: ScoreSide | null; text: ResearchMessages }) {
  const p = text.preview;
  return (
    <section className={r.side} aria-label={title}>
      <h4>{title}</h4>
      {side ? (
        <>
          <div className={r.big} aria-label={p.composite}>{formatNumber(side.composite, 3)}</div>
          <div>{p.flag}: {p.flagLabel[side.flag]}</div>
          <table className="data" aria-label={`${title}: ${p.clips}`}>
            <thead><tr><th>{p.clip}</th><th>{p.subscore}</th></tr></thead>
            <tbody>
              {side.clips.map((c) => (
                <tr key={c.id}>
                  <td>{c.title}{c.lowConfidence ? ` (${p.lowConfidence})` : ""}</td>
                  <td>{formatNumber(c.subscore, 3)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      ) : (
        <p className="muted">{p.computing}</p>
      )}
    </section>
  );
}

/**
 * Published against draft for one session (RUI-3): composite, analysis label, per-clip subscores, the
 * `param_not_applied` knobs, and at frame level the scanpath. The published side never changes with the draft.
 */
export function PreviewCard({ card, text = researchText }: { card: CardData; text?: ResearchMessages }) {
  const p = text.preview;
  const changed = card.published && card.draft && (card.published.composite !== card.draft.composite || card.published.flag !== card.draft.flag);
  return (
    <article className={`${r.card} ${changed ? r.changed : ""}`} aria-label={p.sessionCard(card.id)} data-changed={changed ? "true" : "false"}>
      <header className={r.row}>
        <strong>{card.token}</strong>
        <span className={`badge ${r.badge}`}>{card.framesAvailable ? p.framesBadge : p.scoreBadge}</span>
        <span className="muted">{changed ? p.changed : p.unchanged}</span>
      </header>
      {card.error ? <p role="alert" className="muted">{p.failed(card.error)}</p> : null}
      <div className={r.sides}>
        <Side title={p.published} side={card.published} text={text} />
        <Side title={p.draft} side={card.draft} text={text} />
      </div>
      {card.notApplied.length ? (
        <p>{card.notApplied.map((id) => <span key={id} className={r.chip}>{p.paramNotApplied(id)}</span>)}</p>
      ) : null}
      {card.framesAvailable ? (
        <section aria-label={p.scanpath}>
          <h4>{p.scanpath}</h4>
          <ScanpathPlot published={card.publishedTrace} draft={card.draftTrace} aois={card.aois} text={text} />
          {card.framesNote ? <p className="muted">{p.framesUnavailable}</p> : null}
          {!card.publishedTrace && !card.draftTrace ? <p className="muted">{p.scanpathEmpty}</p> : null}
        </section>
      ) : null}
    </article>
  );
}
