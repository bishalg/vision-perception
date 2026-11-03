import type { Trace } from "@/lib/research/preview";
import { researchText, type ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

export interface PlotRect {
  name: string;
  xMin: number;
  yMin: number;
  xMax: number;
  yMax: number;
}

/** The most trace points drawn per line (down-sampled evenly; the Rust core already thins its trace). */
const MAX_DRAWN_POINTS = 300;
const W = 160;
const H = 90;

function thin<T>(items: readonly T[]): T[] {
  if (items.length <= MAX_DRAWN_POINTS) return [...items];
  const stride = items.length / MAX_DRAWN_POINTS;
  return Array.from({ length: MAX_DRAWN_POINTS }, (_, i) => items[Math.floor(i * stride)]!);
}

const px = (x: number) => (x * W).toFixed(2);
const py = (y: number) => (y * H).toFixed(2);

/**
 * Inline SVG of one clip's gaze trace (RUI-3): a down-sampled polyline with fixation circles and the areas of
 * interest as rectangles, the published and the draft trace overlaid. Coordinates are normalised screen units. Pure
 * markup (no script, no style attribute), so the production CSP has nothing to block. Draws what the Rust core
 * returned; without a trace it draws the frame and areas only.
 */
export function ScanpathPlot({
  published,
  draft,
  aois = [],
  text = researchText,
}: {
  published: Trace | null;
  draft: Trace | null;
  aois?: PlotRect[];
  text?: ResearchMessages;
}) {
  const line = (t: Trace | null) => (t ? thin(t.points).map((p) => `${px(p.x)},${py(p.y)}`).join(" ") : "");
  return (
    <svg className={r.plot} viewBox={`0 0 ${W} ${H}`} role="img" aria-label={text.preview.scanpathLabel} data-testid="scanpath">
      <rect x="0" y="0" width={W} height={H} fill="none" className={r.axis} />
      {aois.map((a) => (
        <rect key={a.name} className={r.plotAoi} x={px(a.xMin)} y={py(a.yMin)} width={px(a.xMax - a.xMin)} height={py(a.yMax - a.yMin)}>
          <title>{a.name}</title>
        </rect>
      ))}
      {published ? <polyline className={`${r.plotTrace} ${r.tracePublished}`} points={line(published)} /> : null}
      {draft ? <polyline className={`${r.plotTrace} ${r.traceDraft}`} points={line(draft)} /> : null}
      {published?.fixations.map((f, i) => <circle key={`p${i}`} className={r.fixPublished} cx={px(f.x)} cy={py(f.y)} r={2} />)}
      {draft?.fixations.map((f, i) => <circle key={`d${i}`} className={r.fixDraft} cx={px(f.x)} cy={py(f.y)} r={1.5} />)}
    </svg>
  );
}
