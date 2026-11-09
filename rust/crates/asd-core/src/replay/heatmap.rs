//! Gaze density grids (ADR-0015 V-3).
//!
//! A grid is `w` x `h` cells over the screen-normalised square [0, 1] x [0, 1], row-major from
//! the top-left (`cells[row * w + col]`), cell centres at `((col + 0.5) / w, (row + 0.5) / h)`.
//! The session density is an isotropic Gaussian kernel density of the valid samples (one
//! sample per valid frame, bandwidth `sigma` in normalised units on both axes), computed as the
//! separable product of two 1-D kernels and scaled so the largest cell is 1. Samples outside
//! the screen still contribute their tails (gaze points are never clamped).

use serde::{Deserialize, Serialize};

use super::ReplayError;
use crate::metrics::{is_valid_frame, media_time};
use crate::norms::DEFAULT_MIN_N;
use crate::types::GazeFrame;

/// Largest grid side (cells).
pub const MAX_GRID_SIDE: u32 = 1024;
/// Default minimum number of sessions in a cohort grid: the PRV-1 / norms small-cell `n`.
pub const COHORT_MIN_N: usize = DEFAULT_MIN_N;

/// A density grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Grid {
    /// Columns.
    pub w: u32,
    /// Rows.
    pub h: u32,
    /// `w * h` values, row-major from the top-left. Session and cohort grids are in [0, 1];
    /// a difference grid in [-1, 1].
    pub cells: Vec<f32>,
    /// Samples behind the grid (summed over a cohort or both sides of a difference).
    pub n_samples: u64,
    /// Session grids behind the grid: 1 for a session, n for a cohort, the sum for a
    /// difference.
    pub n_grids: u32,
}

/// The density of `frames`' valid samples (optionally only those with media time in
/// `[from, to]`), normalised to a maximum of 1; all zeros with no sample.
pub fn heatmap_grid(
    frames: &[GazeFrame],
    w: u32,
    h: u32,
    bandwidth_norm: f64,
    clip_window: Option<(f64, f64)>,
) -> Result<Grid, ReplayError> {
    for (name, side) in [("w", w), ("h", h)] {
        if side == 0 || side > MAX_GRID_SIDE {
            return Err(ReplayError::InvalidArgument(format!(
                "{name} must be 1..={MAX_GRID_SIDE}, got {side}"
            )));
        }
    }
    if !(bandwidth_norm.is_finite() && bandwidth_norm > 0.0) {
        return Err(ReplayError::InvalidArgument(format!(
            "bandwidth must be a positive number, got {bandwidth_norm}"
        )));
    }
    let (w_us, h_us) = (w as usize, h as usize);
    let mut acc = vec![0.0_f64; w_us * h_us];
    let inv_two_var = 1.0 / (2.0 * bandwidth_norm * bandwidth_norm);
    let kernel = |centre: f64, n: usize| -> Vec<f64> {
        (0..n)
            .map(|i| {
                let d = (i as f64 + 0.5) / n as f64 - centre;
                (-d * d * inv_two_var).exp()
            })
            .collect()
    };
    let mut n_samples = 0u64;
    for f in frames {
        if !is_valid_frame(f) {
            continue;
        }
        let (x, y) = (f.gaze.x_norm, f.gaze.y_norm);
        if !(x.is_finite() && y.is_finite()) {
            continue;
        }
        if let Some((from, to)) = clip_window {
            let t = media_time(f);
            if !(t >= from && t <= to) {
                continue;
            }
        }
        n_samples += 1;
        let gx = kernel(x, w_us);
        let gy = kernel(y, h_us);
        for (row, &ky) in gy.iter().enumerate() {
            if ky == 0.0 {
                continue;
            }
            let base = row * w_us;
            for (col, &kx) in gx.iter().enumerate() {
                acc[base + col] += ky * kx;
            }
        }
    }
    let max = acc.iter().copied().fold(0.0_f64, f64::max);
    let cells = if max > 0.0 {
        acc.iter().map(|v| (v / max) as f32).collect()
    } else {
        vec![0.0; w_us * h_us]
    };
    Ok(Grid {
        w,
        h,
        cells,
        n_samples,
        n_grids: 1,
    })
}

/// The mean of session grids, cell by cell (each session weighs the same). `None` (suppressed)
/// with fewer than `max(min_n, 1)` grids, or when the grids' sizes differ.
pub fn cohort_grid(grids: &[Grid], min_n: usize) -> Option<Grid> {
    let first = grids.first()?;
    if grids.len() < min_n.max(1)
        || grids
            .iter()
            .any(|g| g.w != first.w || g.h != first.h || g.cells.len() != first.cells.len())
    {
        return None;
    }
    let mut acc = vec![0.0_f64; first.cells.len()];
    for g in grids {
        for (a, &c) in acc.iter_mut().zip(&g.cells) {
            *a += f64::from(c);
        }
    }
    let n = grids.len() as f64;
    Some(Grid {
        w: first.w,
        h: first.h,
        cells: acc.iter().map(|v| (v / n) as f32).collect(),
        n_samples: grids.iter().map(|g| g.n_samples).sum(),
        n_grids: grids.iter().map(|g| g.n_grids).sum(),
    })
}

/// `a - b` cell by cell, clamped to [-1, 1]; `None` when the sizes differ.
pub fn difference(a: &Grid, b: &Grid) -> Option<Grid> {
    if a.w != b.w || a.h != b.h || a.cells.len() != b.cells.len() {
        return None;
    }
    Some(Grid {
        w: a.w,
        h: a.h,
        cells: a
            .cells
            .iter()
            .zip(&b.cells)
            .map(|(x, y)| (x - y).clamp(-1.0, 1.0))
            .collect(),
        n_samples: a.n_samples + b.n_samples,
        n_grids: a.n_grids + b.n_grids,
    })
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{frame, invalid, t};
    use super::*;

    fn argmax(g: &Grid) -> (usize, usize) {
        let i = g
            .cells
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        (i % g.w as usize, i / g.w as usize)
    }

    #[test]
    fn single_point_peak_and_kernel_shape() {
        // One sample at the centre of cell (2, 1) of a 5 x 4 grid.
        let f = [frame(0.0, 0.5, 0.375)];
        let g = heatmap_grid(&f, 5, 4, 0.1, None).unwrap();
        assert_eq!(g.cells.len(), 20);
        assert_eq!(argmax(&g), (2, 1));
        assert_eq!(g.cells[7], 1.0);
        // The neighbour one column over is 0.2 away: exp(-0.04 / 0.02) = e^-2.
        let expect = (-2.0_f64).exp() as f32;
        assert!((g.cells[8] - expect).abs() < 1e-6);
        assert!((g.cells[6] - expect).abs() < 1e-6);
        // One row down is 0.25 away: exp(-0.0625 / 0.02).
        let expect = (-3.125_f64).exp() as f32;
        assert!((g.cells[12] - expect).abs() < 1e-6);
        assert_eq!(g.n_samples, 1);
    }

    #[test]
    fn two_clusters_peak_at_the_heavier_and_windows_select() {
        let mut f: Vec<GazeFrame> = (0..30).map(|i| frame(t(i), 0.25, 0.25)).collect();
        f.extend((30..40).map(|i| frame(t(i), 0.75, 0.75)));
        f.extend((40..50).map(|i| invalid(t(i))));
        let g = heatmap_grid(&f, 4, 4, 0.05, None).unwrap();
        assert_eq!(g.n_samples, 40);
        // Cell centres 0.125, 0.375, ...: 0.25 is midway, so the four cells around it share
        // the peak.
        for i in [0, 1, 4, 5] {
            assert!((g.cells[i] - 1.0).abs() < 1e-6);
        }
        // The lighter cluster is about 1/3 as dense.
        assert!((g.cells[10] - 1.0 / 3.0).abs() < 1e-3);
        // Only frames from 0.5 s: the light cluster alone.
        let late = heatmap_grid(&f, 4, 4, 0.05, Some((0.5, 10.0))).unwrap();
        assert_eq!(late.n_samples, 10);
        assert_eq!(late.cells[10], 1.0);
        // No sample: zeros.
        let none = heatmap_grid(&f, 4, 4, 0.05, Some((20.0, 30.0))).unwrap();
        assert!(none.cells.iter().all(|&c| c == 0.0));
    }

    #[test]
    fn bad_arguments_are_errors() {
        let f = [frame(0.0, 0.5, 0.5)];
        assert!(heatmap_grid(&f, 0, 4, 0.1, None).is_err());
        assert!(heatmap_grid(&f, 4, MAX_GRID_SIDE + 1, 0.1, None).is_err());
        assert!(heatmap_grid(&f, 4, 4, 0.0, None).is_err());
        assert!(heatmap_grid(&f, 4, 4, f64::NAN, None).is_err());
    }

    fn grid(cells: Vec<f32>) -> Grid {
        Grid {
            w: 2,
            h: 1,
            cells,
            n_samples: 10,
            n_grids: 1,
        }
    }

    #[test]
    fn cohort_is_suppressed_below_min_n_and_averages_above() {
        let grids: Vec<Grid> = (0..12).map(|i| grid(vec![1.0, i as f32 / 11.0])).collect();
        assert!(cohort_grid(&grids[..11], COHORT_MIN_N).is_none());
        let c = cohort_grid(&grids, COHORT_MIN_N).unwrap();
        assert_eq!(c.cells[0], 1.0);
        assert!((c.cells[1] - 0.5).abs() < 1e-6);
        assert_eq!((c.n_grids, c.n_samples), (12, 120));
        assert!(cohort_grid(&[], 0).is_none());
        let mut odd = grids.clone();
        odd[3].w = 3;
        assert!(cohort_grid(&odd, 1).is_none());
    }

    #[test]
    fn difference_is_signed_and_clamped() {
        let d = difference(&grid(vec![1.0, 0.0]), &grid(vec![0.25, 1.0])).unwrap();
        assert_eq!(d.cells, vec![0.75, -1.0]);
        assert_eq!(d.n_grids, 2);
        let wide = Grid {
            w: 1,
            h: 2,
            ..grid(vec![0.0, 0.0])
        };
        assert!(difference(&grid(vec![0.0, 0.0]), &wide).is_none());
    }
}
