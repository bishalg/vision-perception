import { formatPercent } from "@/lib/format";
import type { AgeProfile, AgeProfileExpectation, AgeProfileSide } from "@/lib/research/age-profile";
import { formatValue } from "@/lib/research/registry";
import type { ResearchMessages } from "@/messages/research";
import r from "./research.module.css";

/**
 * The `age_profile` section of a run page (AGE-1, AGE-3): per age band (labelled from the profile's band edges, AGE-2) a
 * table of the expected responses with the cohort's median, the baseline range, the shares on target / monitor /
 * atypical under the published and the experiment baselines, and the percentile of the expected value. Everything is
 * the Rust job's; a band or cell under `max(k, norms.minN)` shows "n < t", never a number.
 */
export function AgeProfileTable({ profile, k, text }: { profile: AgeProfile; k: number; text: ResearchMessages }) {
  const t = text.analysis;
  const threshold = profile.suppression_threshold || Math.max(k, profile.min_n);
  const hidden = text.suppressedCell(threshold);
  return (
    <section className="panel stack" aria-label={t.ageTitle} data-testid="age-profile">
      <h2>{t.ageTitle}</h2>
      <p className="muted">{t.ageIntro}</p>
      <p>{t.baselines(profile.baselines.published, profile.baselines.draft)}</p>
      <p className="muted">{t.threshold(threshold)}</p>
      {profile.bands.map((band) => {
        const title = t.bandTitle(band.band, band.min_months, band.max_months);
        return (
          <section key={band.band} className="stack" aria-label={title} data-band={band.band}>
            <h3>{title}</h3>
            {band.suppressed || band.n == null ? (
              <p className="muted" data-testid="age-band-suppressed">{hidden}</p>
            ) : band.expectations.length === 0 ? (
              <p className="muted">{t.noExpectations}</p>
            ) : (
              <div className="table-wrap">
                <table className={`data ${r.ageTable}`} aria-label={t.ageCaption(band.band)}>
                  <caption className="muted">{t.ageCols.n}: {band.n}</caption>
                  <thead>
                    <tr>
                      <th scope="col">{t.ageCols.expectation}</th>
                      <th scope="col">{t.ageCols.n}</th>
                      <th scope="col">{t.ageCols.median}</th>
                      <th scope="col">{t.ageCols.range}</th>
                      <th scope="col">{t.ageCols.publishedShares}</th>
                      <th scope="col">{t.ageCols.draftShares}</th>
                      <th scope="col">{t.ageCols.publishedPercentile}</th>
                      <th scope="col">{t.ageCols.draftPercentile}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {band.expectations.map((e) => (
                      <Row key={`${e.stimulus_id}/${e.segment_id}/${e.metric_key}`} e={e} hidden={hidden} text={text} />
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>
        );
      })}
    </section>
  );
}

function Row({ e, hidden, text }: { e: AgeProfileExpectation; hidden: string; text: ResearchMessages }) {
  const t = text.analysis;
  const p = e.published;
  const off = (s: AgeProfileSide) => s.suppressed || s.n == null;
  const shares = (s: AgeProfileSide) =>
    off(s) || !s.shares ? hidden : t.shares(formatPercent(s.shares.on_target), formatPercent(s.shares.monitor), formatPercent(s.shares.atypical));
  const pct = (s: AgeProfileSide) => (off(s) ? hidden : s.expected_percentile == null ? "-" : formatValue(Math.round(s.expected_percentile)));
  const median = off(p) || p.dist.median == null ? hidden : `${formatValue(p.dist.median)}${p.dist.iqr ? ` [${formatValue(p.dist.iqr[0])}, ${formatValue(p.dist.iqr[1])}]` : ""}`;
  const range = p.range && p.range.acceptable_low != null && p.range.acceptable_high != null
    ? t.range(formatValue(p.range.acceptable_low), formatValue(p.range.acceptable_high), p.range.expected == null ? "-" : formatValue(p.range.expected))
    : p.range?.expected != null
      ? formatValue(p.range.expected)
      : t.noRange;
  return (
    <tr data-testid="age-row">
      <th scope="row">{t.expectation(e.stimulus_id, e.segment_id, e.metric_key)}</th>
      <td className="num">{off(p) ? hidden : p.n}</td>
      <td className="num">{median}</td>
      <td>{range}</td>
      <td>{shares(p)}</td>
      <td>{shares(e.draft)}</td>
      <td className="num">{pct(p)}</td>
      <td className="num">{pct(e.draft)}</td>
    </tr>
  );
}
