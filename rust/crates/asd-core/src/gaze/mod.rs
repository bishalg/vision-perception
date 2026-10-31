//! owner: WP-G.1
//!
//! Per-frame gaze maths (ADR-0007 D1; spec `docs/spec/modules/asd-gaze-robustness/technical.md`).
//! Pure functions and single-owner state machines: no I/O, no wall clock, no randomness
//! (PIPE-5). Types live in [`crate::types::tracking`].
//!
//! Coordinate frames used throughout:
//! - **camera frame** (input): metres, device-fixed; see [`raycast::CAMERA_AXES`];
//! - **native screen frame**: millimetres from the active area's top-left in the portrait
//!   native orientation, y down (the frame of `DeviceGeometry`);
//! - **oriented screen frame**: the native frame rotated to the interface orientation
//!   ([`geometry::OrientedGeometry`]), millimetres, y down; eye positions add z = distance in
//!   front of the glass;
//! - **screen-normalised space**: the oriented frame divided by its width and height, so
//!   x, y in [0,1] cover the full display. Points are never clamped in the stored path.

pub mod aoi_space;
pub mod calibration;
pub mod confidence;
pub mod drift;
pub mod filter;
pub mod gapfill;
pub mod geometry;
pub mod pipeline;
pub mod raw_log;
pub mod raycast;
pub mod summary;
pub mod track_state;
pub mod validation;

/// Version of the gaze maths, reported as `trackingQuality.pipelineVersion` (QS-1).
pub const PIPELINE_VERSION: &str = "asd-gaze/1.0.0";
