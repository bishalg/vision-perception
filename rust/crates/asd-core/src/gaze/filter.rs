//! owner: WP-G.3
//!
//! 1-Euro filter (Casiez et al., CHI 2012), a port of `OneEuroFilter.swift` (PIPE-3) with
//! identical output on that file's test cases (`tests/golden/gaze/filter/`). The arithmetic
//! is kept in the Swift order so results match bit for bit, including the quirks:
//! - the first sample uses `dt = 1/60 s` and passes through unchanged;
//! - `dt` is clamped below at `1/240 s` (a repeated or backwards timestamp does not spike);
//! - the derivative uses the previous **raw** value, not the previous filtered one;
//! - the cutoff is floored at 0.001 Hz.
//!
//! Differences that are not arithmetic: no lock (the pipeline is single-owner, `&mut self`),
//! and the three parameters come from `TrackingQualityParams.filter` instead of literals.
//! Time is the sample (frame) clock in seconds, never a wall clock (PIPE-5).

use std::f64::consts::PI;

use crate::types::tracking::FilterParams;

/// Exponential smoothing stage (`LowPassFilter` in Swift).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct LowPass {
    last_raw: Option<f64>,
    last_filtered: Option<f64>,
}

impl LowPass {
    fn filter(&mut self, value: f64, alpha: f64) -> f64 {
        let result = match self.last_filtered {
            Some(prev) => alpha * value + (1.0 - alpha) * prev,
            None => value,
        };
        self.last_raw = Some(value);
        self.last_filtered = Some(result);
        result
    }
}

/// Smoothing factor for a cutoff (Hz) at a frame interval (s).
fn alpha(cutoff: f64, dt: f64) -> f64 {
    let te = 1.0 / cutoff.max(0.001);
    let tau = te / (2.0 * PI);
    1.0 / (1.0 + tau / dt)
}

/// One-dimensional 1-Euro filter.
#[derive(Debug, Clone, PartialEq)]
pub struct OneEuro {
    params: FilterParams,
    x: LowPass,
    dx: LowPass,
    last_t: Option<f64>,
}

impl OneEuro {
    pub fn new(params: FilterParams) -> Self {
        Self {
            params,
            x: LowPass::default(),
            dx: LowPass::default(),
            last_t: None,
        }
    }

    /// Filters the next sample taken at `t_sec` (sample clock).
    pub fn filter(&mut self, value: f64, t_sec: f64) -> f64 {
        let dt = match self.last_t {
            Some(last) => (t_sec - last).max(1.0 / 240.0),
            None => 1.0 / 60.0,
        };
        self.last_t = Some(t_sec);
        let dx = self.x.last_raw.map_or(0.0, |prev| (value - prev) / dt);
        let edx = self.dx.filter(dx, alpha(self.params.d_cutoff_hz, dt));
        let cutoff = self.params.min_cutoff_hz + self.params.beta * edx.abs();
        self.x.filter(value, alpha(cutoff, dt))
    }

    /// Forgets every previous sample; the next one passes through.
    pub fn reset(&mut self) {
        self.x = LowPass::default();
        self.dx = LowPass::default();
        self.last_t = None;
    }
}

/// Two independent 1-Euro filters for a screen point (`OneEuroFilter2D`).
#[derive(Debug, Clone, PartialEq)]
pub struct OneEuro2D {
    x: OneEuro,
    y: OneEuro,
}

impl OneEuro2D {
    pub fn new(params: FilterParams) -> Self {
        Self {
            x: OneEuro::new(params),
            y: OneEuro::new(params),
        }
    }

    pub fn filter(&mut self, p: [f64; 2], t_sec: f64) -> [f64; 2] {
        [self.x.filter(p[0], t_sec), self.y.filter(p[1], t_sec)]
    }

    pub fn reset(&mut self) {
        self.x.reset();
        self.y.reset();
    }
}
