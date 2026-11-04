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

/// The parameter registry, exactly `param-registry-v1.json`: `{$id, registryVersion, params}`
/// (PRM-1, RPX-4).
#[wasm_bindgen(js_name = paramRegistry)]
pub fn param_registry() -> String {
    envelope(Ok(api::param_registry()))
}

/// `{valid, problems, fieldProblems}`: the profile decodes as the contract `ScoringProfile`
/// and passes the backend's range checks (asd-contract `profile_checks`); `fieldProblems`
/// are `{path, message}` with the camelCase member path.
#[wasm_bindgen(js_name = validateScoringProfile)]
pub fn validate_scoring_profile(profile_json: &str) -> String {
    envelope(Ok(api::validate_scoring_profile(profile_json)))
}

/// `{valid, problems, fieldProblems}`: the catalog decodes as the asd-core `StimulusCatalog`
/// and passes the backend's value rules (asd-contract `catalog_checks`); `fieldProblems` are
/// `{path, message}` with the wire JSON path (`clips[2].areas[0].xMin`).
#[wasm_bindgen(js_name = validateCatalog)]
pub fn validate_catalog(catalog_json: &str) -> String {
    envelope(Ok(api::validate_catalog(catalog_json)))
}

/// `{valid, problems, fieldProblems}`: asd-core baseline coverage check (`fieldProblems` is
/// empty: the coverage lines name `video/segment/biomarker` in their text). `required_ids_json` is an
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

/// I18N-4: `"matches"`, `"no_scored_clips"`, ... for a flag headline the core writes, or
/// `null` for any other text (render it as sent).
#[wasm_bindgen(js_name = flagCase)]
pub fn flag_case(headline: &str) -> String {
    envelope(Ok(api::flag_case(headline)))
}

/// DSP-3 (WP-X.0c): `{status, comparator, low, high, unit}` of `value` (`undefined`: not
/// observed) for `metric` (`socialPreferencePct`, ..., `validFramePct`) under the profile's
/// `display` and `sessionQuality` sections.
#[wasm_bindgen(js_name = displayBand)]
pub fn display_band(metric: &str, value: Option<f64>, profile_json: &str) -> String {
    envelope(api::display_band(metric, value, profile_json))
}

/// DSP-4 (WP-X.0c): `"optimal"`, `"acceptable"` or `"degraded"` for a session's valid-frame
/// ratio and interruption under the profile's `sessionQuality`.
#[wasm_bindgen(js_name = sessionQualityVerdict)]
pub fn session_quality_verdict(valid_ratio: f64, interrupted: bool, profile_json: &str) -> String {
    envelope(api::session_quality_verdict(
        valid_ratio,
        interrupted,
        profile_json,
    ))
}

/// `[PerVideoMetricSet]` from `[GazeFrame]` (asd-core `metrics::extract`).
#[wasm_bindgen(js_name = extractMetrics)]
pub fn extract_metrics(frames_json: &str) -> String {
    envelope(api::extract_metrics(frames_json))
}

/// Sandbox re-score of a stored session (1.6/1.7/1.8 request or bare `rawPayload` record) with
/// a scoring profile and baseline set: `analysis::evaluate` at `score` level, so the profile's
/// `metrics` and `lowConfidenceValidRatio` apply (WP-X.4). `options_json`: `{"ageMonths": n, "scoredAtUtc"?:
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

/// Sandbox re-score of a session as the console reads it: `session_json` is the API
/// `SessionDetail`, `clips_json` the `items` of `GET /v1/sessions/{id}/clips`.
/// `options_json`: `{"ageMonths": n, "scoredAtUtc"?: "...", "validSampleRatio"?: r}`.
/// Same result as [`rescore_session`] (`metricSource` is `sessionReport`); what the API
/// form cannot carry is listed in `warnings`.
#[wasm_bindgen(js_name = rescoreApiSession)]
pub fn rescore_api_session(
    session_json: &str,
    clips_json: &str,
    profile_json: &str,
    baselines_json: &str,
    options_json: &str,
) -> String {
    envelope(api::rescore_api_session(
        session_json,
        clips_json,
        profile_json,
        baselines_json,
        options_json,
    ))
}

/// RPX-4: re-processes a research frame log (`asd-raw-tracking/1` JSON lines) with a catalog
/// (or `null` when the header carries the AOIs), a scoring profile and a baseline set:
/// `analysis::evaluate` at `frames` level. `options_json`: `{"ageMonths": n, "scoredAtUtc"?,
/// "model"?, "glasses"?, "traceHz"?}`. Result: the core `Evaluation` (`level`, `unscorable`?,
/// `score`, `flag`, `qualityStatus`, `validRatio`, `metrics`, `tracking`, `fixations`?,
/// `trace`?, `warnings`).
#[wasm_bindgen(js_name = reprocessSession)]
pub fn reprocess_session(
    frame_log_jsonl: &str,
    catalog_json: &str,
    profile_json: &str,
    baselines_json: &str,
    options_json: &str,
) -> String {
    envelope(api::reprocess_session(
        frame_log_jsonl,
        catalog_json,
        profile_json,
        baselines_json,
        options_json,
    ))
}
