//! owner: WP-X.1
//!
//! PRV-1: small-cell suppression of a run summary, the one function the job (before
//! `summary` is stored), the cohort preview and the export apply.
//!
//! With `k = organizations.settings.research.small_cell_k`:
//! - every [`Dist`] with `n < k` becomes `{n: null, suppressed: true}` and nothing else;
//!   one with `n < MIN_N_FOR_DECILES` (20) keeps its numbers but loses `p10` / `p90`;
//! - a band (and `overall`) with `n < k` is `suppressed`, its `n` and flag rates `null`;
//! - flag counts and transition cells with `0 < n < k` become `null` (zero stays zero) and
//!   are counted in `counts.suppressed_cells`;
//! - a sweep cell with `n < k` keeps only its `values`.
//!
//! Idempotent: suppressing twice with the same `k` changes nothing the second time.

use super::aggregate::{BandSummary, Dist, FlagCounts, RunSummary, Transition, MIN_N_FOR_DECILES};

/// Applies PRV-1 to `summary` in place and records `k` in `small_cell_k`.
pub fn suppress(summary: &mut RunSummary, k: u32) {
    summary.small_cell_k = k;
    let mut cells = 0u32;

    for band in summary
        .by_age_band
        .iter_mut()
        .chain(std::iter::once(&mut summary.overall))
    {
        suppress_band(band, k);
    }

    suppress_counts(&mut summary.flags.published, k, &mut cells);
    suppress_counts(&mut summary.flags.draft, k, &mut cells);
    suppress_transitions(&mut summary.flags.transitions, k, &mut cells);
    suppress_transitions(&mut summary.quality.transitions, k, &mut cells);

    if let Some(sweep) = summary.sweep.as_mut() {
        for cell in &mut sweep.cells {
            if cell.n.is_some_and(|n| n < k) || cell.n.is_none() {
                let values = std::mem::take(&mut cell.values);
                let quality_keys: Vec<String> = cell.quality_rate.keys().cloned().collect();
                let metric_keys: Vec<String> = cell.metrics.keys().cloned().collect();
                *cell = Default::default();
                cell.values = values;
                cell.quality_rate = quality_keys.into_iter().map(|q| (q, None)).collect();
                cell.metrics = metric_keys
                    .into_iter()
                    .map(|m| (m, Default::default()))
                    .collect();
            }
        }
    }

    summary.counts.suppressed_cells += cells;
}

fn suppress_band(band: &mut BandSummary, k: u32) {
    if band.suppressed || band.n.is_some_and(|n| n < k) {
        band.suppressed = true;
        band.n = None;
        band.flag_rate.published = None;
        band.flag_rate.draft = None;
    }
    for c in band
        .metrics
        .values_mut()
        .chain(std::iter::once(&mut band.composite))
    {
        suppress_dist(&mut c.published, k);
        suppress_dist(&mut c.draft, k);
        suppress_dist(&mut c.delta, k);
    }
}

/// PRV-1 on one distribution.
pub fn suppress_dist(d: &mut Dist, k: u32) {
    if d.suppressed || d.n.is_none_or(|n| n < k) {
        *d = Dist {
            suppressed: true,
            ..Dist::default()
        };
        return;
    }
    if d.n.is_some_and(|n| n < MIN_N_FOR_DECILES) {
        d.p10 = None;
        d.p90 = None;
    }
}

fn small(n: u32, k: u32) -> bool {
    n > 0 && n < k
}

fn suppress_counts(c: &mut FlagCounts, k: u32, cells: &mut u32) {
    for slot in [&mut c.green, &mut c.yellow, &mut c.red, &mut c.unreliable] {
        if slot.is_some_and(|n| small(n, k)) {
            *slot = None;
            *cells += 1;
        }
    }
}

fn suppress_transitions(t: &mut [Transition], k: u32, cells: &mut u32) {
    for (_, _, n) in t.iter_mut() {
        if n.is_some_and(|v| small(v, k)) {
            *n = None;
            *cells += 1;
        }
    }
}
