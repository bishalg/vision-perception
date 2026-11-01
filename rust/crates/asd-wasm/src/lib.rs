//! owner: WP-R.6
//!
//! WebAssembly surface over asd-core and asd-contract for the web console (ADR-0006 §2;
//! feeds the WP-C.5 sandbox re-score and the scoring-profile / baselines editors).
//!
//! Every export takes JSON strings (and scalars) and returns one JSON string, the envelope
//! of [`error::envelope`]: `{"ok":true,"result":...}` or
//! `{"ok":false,"error":{"kind","message"}}`. Nothing is thrown for bad input. The console
//! keeps its own TypeScript types and checks them against the exported JSON Schemas; it
//! never re-implements the scoring.
//!
//! Panics: wasm32 aborts on panic (a `RuntimeError: unreachable` in JS), so a panic cannot
//! become an envelope. The hook installed at start-up logs it with `console.error` and keeps
//! the message for [`last_panic`]; the wrapper should report
//! `{"ok":false,"error":{"kind":"panic","message":lastPanic()}}` and re-instantiate the
//! module. The exports are written not to panic on any input.
//!
//! This file only adapts types; the operations live in [`api`] so native tests run exactly
//! what the exports run.

use std::sync::Mutex;

use wasm_bindgen::prelude::wasm_bindgen;

pub mod api;
pub mod error;
pub mod session;

use error::envelope;

static LAST_PANIC: Mutex<Option<String>> = Mutex::new(None);

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(message: &str);
}

/// Installs the panic hook (idempotent in effect: a second call wraps the first hook, which
/// only records the same message twice). Runs automatically when the module starts.
#[wasm_bindgen(start)]
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info.to_string();
        if let Ok(mut slot) = LAST_PANIC.lock() {
            *slot = Some(message.clone());
        }
        #[cfg(target_arch = "wasm32")]
        console_error(&format!("asd-wasm panic: {message}"));
        previous(info);
    }));
}

/// The message of the last panic since start-up, if any (see the module docs).
#[wasm_bindgen(js_name = lastPanic)]
pub fn last_panic() -> Option<String> {
    LAST_PANIC.lock().ok().and_then(|slot| slot.clone())
}

/// `{coreVersion, sessionSchemaVersions, sessionSchemaIds, scoringProfileSchemaVersion,
/// configBundleSchemaId, lowConfidenceValidRatio}`.
#[wasm_bindgen(js_name = contractInfo)]
pub fn contract_info() -> String {
    envelope(Ok(api::contract_info()))
}

/// The bundled default `ScoringProfile`.
#[wasm_bindgen(js_name = defaultScoringProfile)]
pub fn default_scoring_profile() -> String {
    envelope(Ok(api::default_scoring_profile()))
}

/// `{valid, problems}`: the profile decodes as the contract `ScoringProfile`.
#[wasm_bindgen(js_name = validateScoringProfile)]
pub fn validate_scoring_profile(profile_json: &str) -> String {
    envelope(Ok(api::validate_scoring_profile(profile_json)))
}

/// `{valid, problems}`: asd-core baseline coverage check. `required_ids_json` is an
/// optional JSON array of stimulus ids that must have a profile.
#[wasm_bindgen(js_name = validateBaselines)]
pub fn validate_baselines(baselines_json: &str, required_ids_json: Option<String>) -> String {
    envelope(api::validate_baselines(
        baselines_json,
        required_ids_json.as_deref(),
    ))
}

/// `PerVideoSessionScore` from metric sets, baselines, age and frames (asd-core
/// `scoring::score_session`).
#[wasm_bindgen(js_name = scoreSession)]
pub fn score_session(
    metric_sets_json: &str,
    baselines_json: &str,
    age_months: i32,
    frames_json: &str,
    scored_at_utc: &str,
) -> String {
    envelope(api::score_session(
        metric_sets_json,
        baselines_json,
        i64::from(age_months),
        frames_json,
        scored_at_utc,
    ))
}

/// `AttentionFlagAssessment` of a session score under a profile's flag thresholds.
#[wasm_bindgen(js_name = assessFlag)]
pub fn assess_flag(score_json: &str, profile_json: &str) -> String {
    envelope(api::assess_flag(score_json, profile_json))
}

/// `[PerVideoMetricSet]` from `[GazeFrame]` (asd-core `metrics::extract`).
#[wasm_bindgen(js_name = extractMetrics)]
pub fn extract_metrics(frames_json: &str) -> String {
    envelope(api::extract_metrics(frames_json))
}

/// Sandbox re-score of a stored session (1.6/1.7 request or bare `rawPayload` record) with
/// a scoring profile and baseline set. `options_json`: `{"ageMonths": n, "scoredAtUtc"?:
/// "..."}`. `frames_json` (optional `[GazeFrame]`) selects the exact device path. Result:
/// `{sessionId, schemaVersion, metricSource, score, flag, device, warnings}`.
#[wasm_bindgen(js_name = rescoreSession)]
pub fn rescore_session(
    session_json: &str,
    profile_json: &str,
    baselines_json: &str,
    options_json: &str,
    frames_json: Option<String>,
) -> String {
    envelope(api::rescore_session(
        session_json,
        profile_json,
        baselines_json,
        options_json,
        frames_json.as_deref(),
    ))
}
