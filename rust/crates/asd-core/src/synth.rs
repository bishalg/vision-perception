//! owner: WP-R.4
//!
//! CORE-9: seeded generator of schema-valid session-v1.6 payloads. Rust port of
//! `tools/asd_synth/generator.py` with the CON-13 contract corrections:
//!
//! - `perVideo[].dwell_*` are **ratios of the clip duration** in `[0, 1]` (Python emitted seconds);
//! - `perVideo[].status` is one of `onTarget | monitor | atypical` (Python emitted `offTarget`);
//! - `GENERATOR_VERSION = "2.0"`. Output is *not* byte-compatible with the Python 1.0 generator.
//!
//! The payload conforms to the CON-2 schema `docs/spec/schemas/asd/session-v1.6.json`
//! (`SessionIngestRequest`): `rawPayload` is a full `GazeSessionRecord` (camelCase, with the
//! synthetic markers as extra keys), `perVideo[]` rows carry `order`/`score`/`weight` and a
//! typed `details` block (`dwellUnits: "ratio"`, camelCase `rows[]`), `qualityVerdict` is one of
//! `optimal | acceptable | degraded | rejected`, and unmeasured metrics are absent keys (as
//! Swift encodes them), never `null`.
//!
//! Determinism: output depends only on `(seed, index, profile)`. The RNG is ChaCha20 seeded
//! with `sha256("asd_synth:2.0:{seed}:{index}")`; no wall clock, no global RNG. The derived
//! distributions (uniform, gauss, sample) are implemented here, not taken from a crate, so
//! the stream cannot change under a dependency upgrade.
//!
//! The payload stays a `serde_json::Value`: `asd-contract` depends on `asd-core`, so the typed
//! `SessionIngestRequest` cannot be used here. `asd-cli synth` decodes and schema-validates
//! every payload with `asd-contract` before writing it.

use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::norms::default_age_brackets;
use crate::types::{
    AgeBracket, CaptureSource, ExpectationStatus, NormativeDirection, PerVideoScoreReport,
    PerVideoScoreRow,
};

pub const GENERATOR_VERSION: &str = "2.0";
pub const SCHEMA_VERSION: &str = "1.6";
pub const ID_PREFIX: &str = "synth_";
pub const TOKEN_PREFIX: &str = "synth-";
pub const NOTICE: &str = "SYNTHETIC DATA - not a real child, not for clinical or norm use";
/// `rawPayload.schemaVersion` (record form, `"v1.6"`; the wire form drops the `v`).
pub const RECORD_SCHEMA_VERSION: &str = "v1.6";
/// Fixed session clock origin (2026-01-01T00:00:00Z, Unix seconds): no wall clock.
const EPOCH_UNIX_SEC: i64 = 1_767_225_600;
/// Spacing of consecutive synthetic sessions on the fixed clock.
const SESSION_SPACING_SEC: i64 = 600;

/// `(clip_id, duration_sec, kind)` from the bundled catalog.
pub const CLIPS: [(&str, f64, ClipKind); 8] = [
    ("stim_01_social_peekaboo_8s", 8.0, ClipKind::Social),
    ("stim_02_gaze_cue_right_8s", 8.0, ClipKind::GazeCue),
    ("stim_03_gaze_cue_left_8s", 8.0, ClipKind::GazeCue),
    ("stim_04_surprise_giftbox_10s", 10.0, ClipKind::Social),
    ("stim_05_geopref_split_10s", 10.0, ClipKind::GeoPref),
    ("stim_06_dynamic_gears_marbles_8s", 8.0, ClipKind::Geometric),
    (
        "stim_07_cartoon_slapstick_penguin_8s",
        8.0,
        ClipKind::Social,
    ),
    ("stim_09_social_turn_taking_10s", 10.0, ClipKind::Social),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipKind {
    Social,
    GazeCue,
    GeoPref,
    Geometric,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Typical,
    ReducedSocial,
    LowQuality,
    Short,
}

pub const PROFILES: [Profile; 4] = [
    Profile::Typical,
    Profile::ReducedSocial,
    Profile::LowQuality,
    Profile::Short,
];

/// Cycled so small N covers every profile; typical is repeated to dominate.
const PROFILE_CYCLE: [Profile; 5] = [
    Profile::Typical,
    Profile::Typical,
    Profile::ReducedSocial,
    Profile::LowQuality,
    Profile::Short,
];

impl Profile {
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Typical => "typical",
            Profile::ReducedSocial => "reduced_social",
            Profile::LowQuality => "low_quality",
            Profile::Short => "short",
        }
    }
}

impl std::str::FromStr for Profile {
    type Err = SynthError;
    fn from_str(s: &str) -> Result<Self, SynthError> {
        PROFILES
            .into_iter()
            .find(|p| p.as_str() == s)
            .ok_or_else(|| SynthError::UnknownProfile(s.to_string()))
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SynthError {
    #[error("unknown profile {0:?}; expected one of typical, reduced_social, low_quality, short")]
    UnknownProfile(String),
}

/// Age bands (same as the norm aggregator brackets).
pub fn age_bands() -> Vec<AgeBracket> {
    default_age_brackets()
}

struct Rng(ChaCha20Rng);

impl Rng {
    fn new(seed: u64, index: u64) -> Self {
        let digest = Sha256::digest(format!("asd_synth:{GENERATOR_VERSION}:{seed}:{index}"));
        let mut s = [0u8; 32];
        s.copy_from_slice(&digest);
        Rng(ChaCha20Rng::from_seed(s))
    }
    /// Uniform in [0, 1) with 53 bits.
    fn unit(&mut self) -> f64 {
        (self.0.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.unit()
    }
    /// Inclusive integer range.
    fn randint(&mut self, a: i64, b: i64) -> i64 {
        a + (self.unit() * (b - a + 1) as f64) as i64
    }
    /// Box-Muller.
    fn gauss(&mut self, mu: f64, sigma: f64) -> f64 {
        let u1 = 1.0 - self.unit();
        let u2 = self.unit();
        mu + sigma * (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
    /// `k` distinct indices out of `n` (partial Fisher-Yates), in draw order.
    fn sample(&mut self, n: usize, k: usize) -> Vec<usize> {
        let mut pool: Vec<usize> = (0..n).collect();
        for i in 0..k.min(n) {
            let j = i + (self.unit() * (n - i) as f64) as usize;
            pool.swap(i, j);
        }
        pool.truncate(k.min(n));
        pool
    }
}

fn r(x: f64, digits: i32) -> f64 {
    let m = 10f64.powi(digits);
    (x * m).round() / m
}

/// RFC 3339 UTC with milliseconds (`2026-01-01T00:00:00.000Z`), as `SessionPayloadCoding`
/// writes dates. Civil-from-days after H. Hinnant; valid for any date after 1970.
fn rfc3339_ms(unix_ms: i64) -> String {
    let (secs, ms) = (unix_ms.div_euclid(1000), unix_ms.rem_euclid(1000));
    let (days, sod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{ms:03}Z",
        sod / 3600,
        sod % 3600 / 60,
        sod % 60
    )
}

/// Inserts `key: value` only when the value is measured (Swift omits unmeasured keys).
fn put_opt(map: &mut Map<String, Value>, key: &str, value: Option<f64>) {
    if let Some(v) = value {
        map.insert(key.into(), json!(v));
    }
}

struct Clip {
    json: Value,
    score_row: PerVideoScoreReport,
    social_sec: f64,
    geometric_sec: f64,
    eye_sec: f64,
    mouth_sec: f64,
    latency: Option<f64>,
}

fn clip_score(rng: &mut Rng, order: usize, clip_idx: usize, profile: Profile, valid: f64) -> Clip {
    let (clip_id, dur, kind) = CLIPS[clip_idx];
    let mut social_bias = match profile {
        Profile::Typical => 0.65,
        Profile::ReducedSocial => 0.25,
        _ => 0.5,
    };
    if kind == ClipKind::Geometric {
        social_bias = 1.0 - social_bias;
    }
    let on_screen = dur * valid.clamp(0.0, 1.0) * rng.uniform(0.8, 0.97);
    let frac = rng.gauss(social_bias, 0.08).clamp(0.02, 0.98);
    let social = on_screen * frac;
    let geometric = on_screen * (1.0 - frac) * rng.uniform(0.5, 0.95);
    let eye_share = if profile == Profile::Typical {
        rng.uniform(0.3, 0.6)
    } else {
        rng.uniform(0.1, 0.4)
    };
    let eye = social * eye_share;
    let mouth = social * rng.uniform(0.1, 0.3);
    let fixations = ((on_screen / rng.uniform(0.2, 0.45)) as i64).max(1);
    let imputed = profile == Profile::LowQuality && rng.unit() < 0.4;
    let lat_raw = rng.uniform(250.0, 1200.0)
        * if profile == Profile::ReducedSocial {
            1.5
        } else {
            1.0
        };
    let latency = if imputed { None } else { Some(r(lat_raw, 1)) };
    // CON-13: only the three statuses the iOS client emits.
    let status = if imputed {
        ExpectationStatus::Atypical
    } else if frac >= 0.4 {
        ExpectationStatus::OnTarget
    } else if frac >= 0.25 {
        ExpectationStatus::Monitor
    } else {
        ExpectationStatus::Atypical
    };
    let status_str = match status {
        ExpectationStatus::OnTarget => "onTarget",
        ExpectationStatus::Monitor => "monitor",
        ExpectationStatus::Atypical => "atypical",
    };
    let score = r((frac * (0.6 + 0.4 * valid)).clamp(0.0, 1.0), 3);

    // Ratios of clip duration (CON-13).
    let ratio = |sec: f64| r((sec / dur).clamp(0.0, 1.0), 3);

    // camelCase rows for rawPayload.perVideo (trap 4: a different struct from `perVideo`).
    let mk = |biomarker: &str, observed: f64, imputed: bool, direction: NormativeDirection| {
        PerVideoScoreRow {
            segment_id: "clip".into(),
            segment_description: "Whole clip".into(),
            biomarker: biomarker.into(),
            target_aoi: None,
            metric_key: biomarker.into(),
            direction,
            observed,
            expected: None,
            acceptable_low: None,
            acceptable_high: None,
            status,
            score,
            imputed,
            citation: NOTICE.into(),
            source: "synthetic".into(),
        }
    };
    let mut rows = vec![mk(
        "social_dwell_ratio",
        r(social / on_screen.max(1e-9), 4),
        false,
        NormativeDirection::HigherBetter,
    )];
    if social > 0.0 {
        rows.push(mk(
            "eye_dwell_ratio",
            r(eye / social, 4),
            false,
            NormativeDirection::HigherBetter,
        ));
    }
    rows.push(mk(
        "first_entry_latency_ms",
        latency.unwrap_or(dur * 1000.0),
        imputed,
        NormativeDirection::LowerBetter,
    ));
    let score_row = PerVideoScoreReport {
        stimulus_id: clip_id.into(),
        title: clip_id.into(),
        category: "Synthetic".into(),
        rows,
        subscore: score,
        valid_sample_ratio: r(valid, 3),
        low_confidence: profile == Profile::LowQuality,
        has_baseline: false,
        weight: 1.0,
    };

    // `SessionPerVideoRow` (snake_case) with its camelCase `details` (trap 4).
    let mut details = serde_json::to_value(&score_row).expect("report serialises");
    if let Value::Object(d) = &mut details {
        d.remove("stimulusId");
        d.remove("weight");
        d.insert("dwellUnits".into(), json!("ratio"));
    }
    let mut row = Map::new();
    row.insert("clip_id".into(), json!(clip_id));
    row.insert("order".into(), json!(order));
    row.insert("dwell_social".into(), json!(ratio(social)));
    row.insert("dwell_geometric".into(), json!(ratio(geometric)));
    row.insert("dwell_eye".into(), json!(ratio(eye)));
    row.insert("dwell_mouth".into(), json!(ratio(mouth)));
    put_opt(&mut row, "orienting_latency_ms", latency);
    row.insert("fixation_count".into(), json!(fixations));
    row.insert(
        "mean_fixation_ms".into(),
        json!(r(on_screen / fixations as f64 * 1000.0, 1)),
    );
    row.insert("status".into(), json!(status_str));
    row.insert("score".into(), json!(score));
    row.insert("weight".into(), json!(1.0));
    row.insert("details".into(), details);
    let json = Value::Object(row);
    Clip {
        json,
        score_row,
        social_sec: social,
        geometric_sec: geometric,
        eye_sec: eye,
        mouth_sec: mouth,
        latency,
    }
}

/// One synthetic session payload; identical for identical `(seed, index, profile)`.
///
/// `profile = None` picks from the index cycle so small counts cover every profile.
pub fn generate_session(seed: u64, index: u64, profile: Option<Profile>) -> Value {
    let mut rng = Rng::new(seed, index);
    let profile = profile.unwrap_or(PROFILE_CYCLE[(index % PROFILE_CYCLE.len() as u64) as usize]);

    let bands = age_bands();
    let band = &bands[(index % bands.len() as u64) as usize];
    let age = rng.randint(band.min_months, band.max_months);
    let express = rng.unit() < 0.6;
    let (battery, mut n_clips) = if express {
        ("express_30s", 4usize)
    } else {
        ("comprehensive_60s", 8usize)
    };
    if profile == Profile::Short {
        n_clips = (n_clips / 2).max(1);
    }
    let picks = rng.sample(CLIPS.len(), n_clips.min(CLIPS.len()));

    // `GazeQualityVerdict`: optimal | acceptable | degraded | rejected.
    let (mut valid, mut verdict, flag);
    if profile == Profile::LowQuality {
        valid = rng.uniform(0.15, 0.5);
        verdict = "rejected";
        flag = "yellow";
    } else {
        valid = rng.uniform(0.82, 0.99);
        verdict = if valid > 0.9 {
            "optimal"
        } else if rng.unit() < 0.5 {
            "acceptable"
        } else {
            "degraded"
        };
        flag = if profile == Profile::Typical {
            "green"
        } else if rng.unit() < 0.5 {
            "yellow"
        } else {
            "red"
        };
    }
    if profile == Profile::Short {
        valid = rng.uniform(0.6, 0.9);
        verdict = "degraded";
    }

    let clips: Vec<Clip> = picks
        .iter()
        .enumerate()
        .map(|(i, &c)| clip_score(&mut rng, i + 1, c, profile, valid))
        .collect();
    let total_dur: f64 = picks.iter().map(|&c| CLIPS[c].1).sum();
    let duration = r(
        total_dur * if profile == Profile::Short { 0.6 } else { 1.0 } + rng.uniform(0.0, 2.0),
        2,
    );
    let sum = |f: fn(&Clip) -> f64| clips.iter().map(f).sum::<f64>();
    let (social, geometric) = (sum(|c| c.social_sec), sum(|c| c.geometric_sec));
    let (eye, mouth) = (sum(|c| c.eye_sec), sum(|c| c.mouth_sec));
    let min_latency = clips
        .iter()
        .filter_map(|c| c.latency)
        .fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.min(x))));
    let total = social + geometric;
    let distance = r(rng.uniform(30.0, 55.0), 1);

    let composite = if clips.is_empty() {
        0.0
    } else {
        r(
            clips.iter().map(|c| c.score_row.subscore).sum::<f64>() / clips.len() as f64,
            3,
        )
    };
    // Affect summaries computed on device from blend shapes; drawn here (CON-1).
    let smile = r(
        if profile == Profile::ReducedSocial {
            rng.uniform(0.05, 0.3)
        } else {
            rng.uniform(0.2, 0.6)
        },
        3,
    );
    let reactivity = r(rng.uniform(0.05, 0.5), 3);
    let flat_affect = r((1.0 - smile - rng.uniform(0.0, 0.2)).clamp(0.0, 1.0), 3);
    let surprise = r(rng.uniform(0.05, 0.4), 3);

    let session_id = format!("{ID_PREFIX}{seed}_{index:05}");
    let token = format!("{TOKEN_PREFIX}{seed}-{index:05}");
    let capture = serde_json::to_value(CaptureSource::Synthetic).expect("enum");
    let started_ms = (EPOCH_UNIX_SEC + index as i64 * SESSION_SPACING_SEC) * 1000;
    let ended_ms = started_ms + (duration * 1000.0).round() as i64;
    let social_pref = r(if total > 0.0 { social / total } else { 0.5 }, 3);
    let eye_ratio = (social > 0.0).then(|| r(eye / social, 3));
    let mouth_ratio = (social > 0.0).then(|| r(mouth / social, 3));
    let first_latency = min_latency.map(|x| r(x, 1));

    // `rawPayload`: the on-device `GazeSessionRecord`, plus synthetic markers as extra keys.
    let mut raw = Map::new();
    raw.insert(
        "id".into(),
        json!(format!("{ID_PREFIX}rec_{seed}_{index:05}")),
    );
    raw.insert("sessionId".into(), json!(session_id));
    raw.insert("participantId".into(), json!(token));
    raw.insert("captureSource".into(), capture.clone());
    raw.insert("startedAt".into(), json!(rfc3339_ms(started_ms)));
    raw.insert("endedAt".into(), json!(rfc3339_ms(ended_ms)));
    raw.insert("durationSec".into(), json!(duration));
    raw.insert("socialPreferenceScore".into(), json!(social_pref));
    raw.insert("validFrameRatio".into(), json!(r(valid, 3)));
    put_opt(&mut raw, "eyeDwellRatio", eye_ratio);
    put_opt(&mut raw, "mouthDwellRatio", mouth_ratio);
    put_opt(&mut raw, "firstEyeLatencyMs", first_latency);
    raw.insert("calibratedDistanceCm".into(), json!(distance));
    raw.insert("qualityVerdict".into(), json!(verdict));
    raw.insert("socialSmileIntensity".into(), json!(smile));
    raw.insert("smileReactivityIndex".into(), json!(reactivity));
    raw.insert("flatAffectScore".into(), json!(flat_affect));
    raw.insert("surpriseScoreMean".into(), json!(surprise));
    raw.insert("notes".into(), json!(NOTICE));
    raw.insert("schemaVersion".into(), json!(RECORD_SCHEMA_VERSION));
    raw.insert("zeroRawVideoEnforced".into(), json!(true));
    raw.insert("perVideoComposite".into(), json!(composite));
    raw.insert(
        "perVideo".into(),
        serde_json::to_value(clips.iter().map(|c| &c.score_row).collect::<Vec<_>>())
            .expect("reports serialise"),
    );
    raw.insert("synthetic".into(), json!(true));
    raw.insert("generator".into(), json!("asd-cli synth"));
    raw.insert("generatorVersion".into(), json!(GENERATOR_VERSION));
    raw.insert("seed".into(), json!(seed));
    raw.insert("index".into(), json!(index));
    raw.insert("profile".into(), json!(profile.as_str()));
    raw.insert("ageMonths".into(), json!(age));
    raw.insert("ageBand".into(), json!(band.id));
    raw.insert("notice".into(), json!(NOTICE));

    let mut out = Map::new();
    out.insert("sessionId".into(), json!(session_id));
    out.insert("participantToken".into(), json!(token));
    out.insert("captureSource".into(), capture);
    out.insert("durationSec".into(), json!(duration));
    out.insert("validRatio".into(), json!(r(valid, 3)));
    out.insert("socialPreferenceScore".into(), json!(social_pref));
    put_opt(&mut out, "eyeDwellRatio", eye_ratio);
    put_opt(&mut out, "mouthDwellRatio", mouth_ratio);
    put_opt(&mut out, "firstEyeLatencyMs", first_latency);
    out.insert("qualityVerdict".into(), json!(verdict));
    out.insert("calibratedDistanceCm".into(), json!(distance));
    out.insert("attentionFlag".into(), json!(flag));
    out.insert("battery".into(), json!(battery));
    out.insert("schemaVersion".into(), json!(SCHEMA_VERSION));
    out.insert(
        "perVideo".into(),
        Value::Array(clips.iter().map(|c| c.json.clone()).collect()),
    );
    out.insert("rawPayload".into(), Value::Object(raw));
    Value::Object(out)
}

/// `n` sessions with indices `0..n` and the index-cycled profiles.
pub fn generate_sessions(n: u64, seed: u64) -> Vec<Value> {
    (0..n).map(|i| generate_session(seed, i, None)).collect()
}

/// True when the payload carries every synthetic marker.
pub fn is_synthetic(payload: &Value) -> bool {
    payload
        .get("sessionId")
        .and_then(Value::as_str)
        .is_some_and(|s| s.starts_with(ID_PREFIX))
        && payload.get("captureSource").and_then(Value::as_str) == Some("synthetic")
        && payload
            .pointer("/rawPayload/synthetic")
            .and_then(Value::as_bool)
            == Some(true)
}
