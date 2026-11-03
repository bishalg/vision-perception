/**
 * Reads the published acceptable range of one metric for one age band out of a `NormativeBaselineSet` document
 * (`config-bundle-v1.json`: `videos.<clip>.segments[].expectations[]`, each with `biomarker` and `byAge.<band>|default`
 * of `{ acceptableLow, acceptableHigh, expected }`). The console holds the document as `unknown` (the Rust core owns its
 * rules), so the reader is tolerant: anything it cannot read as one clear range yields `null` and the chart shades
 * nothing. It never invents a number and never combines ranges.
 *
 * A metric can appear in several clips and segments. The reader answers only when every occurrence for the band (or
 * its `default`) states the same range; differing ranges are ambiguous, so the answer is `null`.
 */
export interface BaselineRange {
  low: number;
  high: number;
  expected: number | null;
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const num = (v: unknown): number | null => (typeof v === "number" && Number.isFinite(v) ? v : null);

function rangeFor(byAge: unknown, band: string): BaselineRange | null {
  if (!isRecord(byAge)) return null;
  const entry = isRecord(byAge[band]) ? byAge[band] : isRecord(byAge.default) ? byAge.default : null;
  if (!isRecord(entry)) return null;
  const low = num(entry.acceptableLow);
  const high = num(entry.acceptableHigh);
  if (low == null || high == null || low > high) return null;
  return { low, high, expected: num(entry.expected) };
}

export function baselineRange(body: unknown, metric: string, band: string): BaselineRange | null {
  if (!isRecord(body) || !isRecord(body.videos)) return null;
  const found: BaselineRange[] = [];
  for (const video of Object.values(body.videos)) {
    if (!isRecord(video) || !Array.isArray(video.segments)) continue;
    for (const segment of video.segments) {
      if (!isRecord(segment) || !Array.isArray(segment.expectations)) continue;
      for (const exp of segment.expectations) {
        if (!isRecord(exp) || exp.biomarker !== metric) continue;
        const range = rangeFor(exp.byAge, band);
        if (range) found.push(range);
      }
    }
  }
  const first = found[0];
  if (!first) return null;
  const same = found.every((r) => r.low === first.low && r.high === first.high);
  return same ? first : null;
}

/** `trackingQuality.drift.warnDeg|hardDeg` of a scoring-profile document; a missing or non-numeric key is `null`. */
export function driftThresholds(profileBody: unknown): { warnDeg: number | null; hardDeg: number | null } {
  const drift = isRecord(profileBody) && isRecord(profileBody.trackingQuality) ? profileBody.trackingQuality.drift : null;
  if (!isRecord(drift)) return { warnDeg: null, hardDeg: null };
  return { warnDeg: num(drift.warnDeg), hardDeg: num(drift.hardDeg) };
}
