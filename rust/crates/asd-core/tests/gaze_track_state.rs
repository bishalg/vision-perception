//! owner: WP-G.3
//!
//! TRK-1 track states: `lost`, `blink` (with a time-based post-blink recovery, fixes G-F8),
//! `artifact` (eye speed, bad ray), `offscreen` (calibrated point, TRK-2), else `tracked`.

use asd_core::gaze::track_state::{is_offscreen, TrackInputs, TrackStateMachine};
use asd_core::types::tracking::{RayStatus, TrackState, TrackStateParams};

const BLINK_THRESHOLD: f64 = 0.45;

fn machine() -> TrackStateMachine {
    TrackStateMachine::new(TrackStateParams::default(), BLINK_THRESHOLD)
}

fn open(t: f64) -> TrackInputs {
    TrackInputs {
        t_sec: t,
        face: true,
        left_eye_openness: 0.9,
        right_eye_openness: 0.9,
        ray_status: Some(RayStatus::Ok),
        point_norm: Some([0.5, 0.5]),
        eye_speed_deg_per_sec: Some(5.0),
    }
}

fn closed(t: f64) -> TrackInputs {
    TrackInputs {
        left_eye_openness: 0.1,
        ..open(t)
    }
}

#[test]
fn open_eyes_on_screen_are_tracked() {
    assert_eq!(machine().classify(&open(0.0)), TrackState::Tracked);
}

#[test]
fn no_face_is_lost_and_wins_over_everything() {
    let mut m = machine();
    let lost = TrackInputs {
        face: false,
        left_eye_openness: 0.0,
        ray_status: None,
        point_norm: None,
        eye_speed_deg_per_sec: None,
        ..open(0.0)
    };
    assert_eq!(m.classify(&lost), TrackState::Lost);
}

#[test]
fn either_eye_below_threshold_is_blink_strictly() {
    let mut m = machine();
    let at = TrackInputs {
        right_eye_openness: BLINK_THRESHOLD,
        ..open(0.0)
    };
    // The gate's rule is `openness < blinkThreshold`: exactly at the threshold is open.
    assert_eq!(m.classify(&at), TrackState::Tracked);
    let below = TrackInputs {
        right_eye_openness: BLINK_THRESHOLD - 1e-9,
        ..open(1.0)
    };
    assert_eq!(m.classify(&below), TrackState::Blink);
}

/// Recovery is 50 ms of frame time at both frame rates (G-F8: the Swift frame count gave
/// 50 ms at 60 Hz and 100 ms at 30 Hz).
#[test]
fn blink_recovery_is_50_ms_at_30_and_60_hz() {
    for hz in [30.0, 60.0] {
        let dt = 1.0 / hz;
        let mut m = machine();
        let mut t = 10.0;
        for _ in 0..5 {
            assert_eq!(m.classify(&open(t)), TrackState::Tracked);
            t += dt;
        }
        let mut last_blink = t;
        for _ in 0..4 {
            assert_eq!(m.classify(&closed(t)), TrackState::Blink);
            last_blink = t;
            t += dt;
        }
        let mut recovery_frames = 0;
        let mut first_tracked_after = None;
        for _ in 0..10 {
            let s = m.classify(&open(t));
            let since_ms = (t - last_blink) * 1000.0;
            if since_ms <= 50.0 + 1e-6 {
                assert_eq!(
                    s,
                    TrackState::Blink,
                    "{hz} Hz, {since_ms} ms after the blink"
                );
                recovery_frames += 1;
            } else {
                assert_eq!(
                    s,
                    TrackState::Tracked,
                    "{hz} Hz, {since_ms} ms after the blink"
                );
                first_tracked_after.get_or_insert(since_ms);
            }
            t += dt;
        }
        let expected_frames = if hz == 60.0 { 3 } else { 1 };
        assert_eq!(recovery_frames, expected_frames, "{hz} Hz");
        assert!(first_tracked_after.unwrap() > 50.0);
    }
}

#[test]
fn a_new_blink_during_recovery_restarts_it() {
    let mut m = machine();
    m.classify(&closed(0.0));
    assert_eq!(m.classify(&open(0.03)), TrackState::Blink);
    m.classify(&closed(0.04));
    assert_eq!(m.classify(&open(0.08)), TrackState::Blink); // 40 ms after the second blink
    assert_eq!(m.classify(&open(0.1)), TrackState::Tracked); // 60 ms
}

#[test]
fn lost_frames_do_not_extend_or_cancel_recovery() {
    let mut m = machine();
    m.classify(&closed(0.0));
    let lost = TrackInputs {
        face: false,
        ..open(0.02)
    };
    assert_eq!(m.classify(&lost), TrackState::Lost);
    assert_eq!(m.classify(&open(0.04)), TrackState::Blink);
    assert_eq!(m.classify(&open(0.06)), TrackState::Tracked);
}

#[test]
fn implausible_eye_speed_or_bad_ray_is_artifact() {
    let mut m = machine();
    let fast = TrackInputs {
        eye_speed_deg_per_sec: Some(1000.5),
        ..open(0.0)
    };
    assert_eq!(m.classify(&fast), TrackState::Artifact);
    let at_limit = TrackInputs {
        eye_speed_deg_per_sec: Some(1000.0),
        ..open(0.1)
    };
    assert_eq!(m.classify(&at_limit), TrackState::Tracked);
    for status in [
        RayStatus::Backward,
        RayStatus::Grazing,
        RayStatus::OutOfRange,
    ] {
        let bad = TrackInputs {
            ray_status: Some(status),
            point_norm: None,
            ..open(0.2)
        };
        assert_eq!(m.classify(&bad), TrackState::Artifact, "{status:?}");
    }
    // A face without a ray result (no look-at point) is an artefact, not tracked.
    let none = TrackInputs {
        ray_status: None,
        point_norm: None,
        ..open(0.3)
    };
    assert_eq!(m.classify(&none), TrackState::Artifact);
}

#[test]
fn offscreen_is_decided_on_the_calibrated_point_with_margin() {
    let margin = TrackStateParams::default().offscreen_margin_norm;
    assert!(!is_offscreen([-0.02, 1.02], margin));
    assert!(!is_offscreen([0.0, 1.0], margin));
    assert!(is_offscreen([-0.0201, 0.5], margin));
    assert!(is_offscreen([0.5, 1.0201], margin));
    assert!(is_offscreen([f64::NAN, 0.5], margin));

    let mut m = machine();
    let off = TrackInputs {
        point_norm: Some([1.1, 0.5]),
        ..open(0.0)
    };
    assert_eq!(m.classify(&off), TrackState::Offscreen);
    // Blink wins over offscreen.
    let off_blink = TrackInputs {
        point_norm: Some([1.1, 0.5]),
        ..closed(1.0)
    };
    assert_eq!(m.classify(&off_blink), TrackState::Blink);
}

#[test]
fn reset_clears_the_recovery_timer() {
    let mut m = machine();
    m.classify(&closed(0.0));
    m.reset();
    assert_eq!(m.classify(&open(0.01)), TrackState::Tracked);
}
