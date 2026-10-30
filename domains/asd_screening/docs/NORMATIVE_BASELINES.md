# Normative Baselines — Per-Stimulus Expected-vs-Recorded Scoring

**Status: PROVISIONAL — literature-seeded. Not a diagnostic instrument. Not empirically validated on device-captured data.**

This document specifies the per-video expected-vs-recorded scoring matrix: how each stimulus
video maps to its content, what response is expected for the child's age bracket, what the
app measures, and how the recorded values map onto score bands.

## 1. Pipeline overview

```
Video (stim_XX) ──► content segments (what is shown, when)
                         │
                         ▼
        normative_baselines.json ──► expectations per segment × biomarker × age bracket
                         │                              ▲ citations
60 Hz frames (tagged stimulusId + AVPlayer media time)
                         │
                         ▼
        PerVideoMetricsExtractor ──► per-clip metric sets
                         │
                         ▼
        PerVideoScoringEngine ──► matrix rows (observed vs expected, status, score)
                         │
              per-video subscore ──► weighted session composite
```

| Layer | Code |
|---|---|
| Frame tagging | `GazeTelemetrySample.stimulusId` / `.clipMediaTimeSec` (CurvAsdGazeKit), AVPlayer periodic time observer (`StimulusVideoPlayerView`) |
| Extraction | `PerVideoMetrics.swift` (`PerVideoMetricsExtractor`) |
| Baseline model | `Resources/normative_baselines.json` + `NormativeBaselineLibrary.swift` |
| Scoring | `PerVideoScoringEngine.swift` |
| UI | `PerVideoResultViews.swift` (summary card → sub-reports landing → per-video detail pages) |
| Persistence | `GazeSessionRecord` v1.5, vault `per_video_score` line, ResearchKit JSON `per_video` block, FHIR local-code components, doctor PDF §2 |

## 2. Baseline JSON schema

```jsonc
{
  "schemaVersion": "1.0.0",
  "status": "provisional-literature-seeded",     // surfaced in UI + exports
  "ageBrackets": [ { "id": "18-23", "minMonths": 18, "maxMonths": 23 }, ... ],
  "videos": {
    "<stimulusId>": {
      "title": "...", "category": "...",
      "weight": 0.30,                            // relative composite weight
      "segments": [                              // what the feed shows, in which duration chunk
        {
          "id": "gaze_follow_right",
          "description": "Actor turns gaze to the right-side target object.",
          "startSec": 3.5, "endSec": 8.0,
          "expectations": [
            {
              "biomarker": "first_entry_latency_ms",   // matches extractor metric keys
              "direction": "lowerBetter",              // higherBetter | lowerBetter | band
              "targetAOI": "Gaze_Target_Right",        // composes "biomarker::AOI" lookup key
              "byAge": {
                "default": { "expected": 600.0, "acceptableHigh": 1200.0 },
                "12-17":   { "expected": 800.0, "acceptableHigh": 1500.0 }   // bracket override
              },
              "citation": "...",
              "source": "literature"                   // literature | interpolated | collected-candidate
            }
          ]
        }
      ]
    }
  }
}
```

Resolution rules:
- Bracket id wins over `"default"`; out-of-range ages clamp to the nearest bracket.
- `source: "literature"` rows carry a published anchor; `"interpolated"` rows are developmental
  interpolations and are labeled as such in the UI's Basis section.
- Additive fields only: every optional field decodes with `decodeIfPresent`.

## 3. Metric vocabulary (extractor output)

AOI-specific metrics are keyed `biomarker::AOI`:

| Metric | Meaning |
|---|---|
| `aoi_dwell_ratio::<AOI>` | valid frames in AOI / valid frames in clip |
| `first_entry_latency_ms::<AOI>` | earliest clip media time the AOI was fixated; **missing key = never fixated** (scoring imputes clip duration, worst case, `imputed: true`) |
| `social_dwell_ratio`, `geometric_dwell_ratio`, `social_preference_score` | GeoPref pair (self-describing; no AOI suffix) |
| `eye_dwell_ratio`, `mouth_dwell_ratio` | share of eye/mouth hits among face-region hits |
| `fixation_count`, `mean_fixation_duration_ms` | lightweight I-VT (5.0 normalized u/s threshold, 80 ms minimum — unit-corrected from the C++ pixel-space 150 px/s) |
| `social_smile_intensity_mean`, `smile_reactivity_index` (peak − first-500 ms baseline), `flat_affect_score`, `surprise_score_mean` | facial affect |
| `valid_sample_ratio` | quality gate input (not a scored expectation) |

## 4. Score mapping

Per expectation (`PerVideoScoringEngine.evaluate`):
- `higherBetter`: onTarget ≥ expected; monitor ≥ acceptableLow; atypical below.
- `lowerBetter`: onTarget ≤ expected; monitor ≤ acceptableHigh; atypical above.
- `band`: onTarget within [acceptableLow, acceptableHigh]; monitor within 1 band-width outside.
- Continuous score: piecewise-linear 1 → 0 from the expected edge to one band-width beyond tolerance.

Per video: subscore = mean of row scores. Session composite = Σ(weightᵢ × subscoreᵢ) / Σweightᵢ over
clips **that ran and have baseline profiles**. Clips below the 0.60 valid-sample gate are marked
`lowConfidence` (scores shown, flagged as indicative only). Untagged frames group under
`session_untagged` and are excluded from the composite.

The existing session-level `BiomarkerClassificationEngine` (and its CoreML hook) is **unchanged**;
the matrix composite displays alongside it by design until validated.

## 5. Literature anchors used (v1.0.0)

| Anchor | Value | Source |
|---|---|---|
| Typical social-side fixation | > 70% | Pierce et al., eLife 2019 (GeoPref) |
| ASD-Geo subtype flag | geometric ≥ 69% (98–99% specificity); band flags > 60% | Pierce et al., eLife 2019 |
| First-eye latency | < 420 ms typical, > 600 ms prolonged | Curv clinical baseline; Jones et al. JAMA 2023 (TFF features) |
| Eye-region dwell share | band 0.25–0.45 | Curv clinical baseline; JBI 2022 feature set |
| Smile reactivity | < 0.20 attenuated | Curv baseline; Perochon et al. NEJM AI 2024 |
| Flat affect | > 0.30 at-risk | Curv baseline; DATA_STORAGE_AND_PIPELINE D3.3 |
| Valid-sample gate | ≥ 0.60 low-confidence marking | JBI 2022 (> 75% for full validity) |
| Age bracketing directive | age-stratified norms | JBI 2022; ML_ROADMAP §3 (μ_typical(Age)) |

All other cells are `source: "interpolated"` with the basis stated in their citation string.

## 6. Building our own norms (collection mode)

1. Caregiver enables **Settings → Privacy & Security → Normative Data Collection** (requires
   research consent via `ResearchKitStudyStore`; live TrueDepth only — simulation is always excluded
   by `NormCollectionGate`).
2. Opted-in sessions write a `norm_collection_meta` vault line (consent version, age bracket,
   device model — de-identified) alongside the `per_video_score` line.
3. Exports flow through the existing ResearchKit sync queue to the research server
   (`POST /api/v1/batch/sync`, stored as-is — schema-on-read).
4. `cargo run -p asd-cli -- norms aggregate --input <export-dir> --output <file>` aggregates to a
   **candidate** baseline: per cell n / median / p10-or-p90, with a **min n = 12 guard** and
   imputed rows excluded. Cells below the guard are withheld (null).
5. A clinician/researcher reviews `candidate_normative_baselines.json`, then promotes it by
   bumping the bundled file's `schemaVersion` and flipping `status`. Never promote without review.

## 7. Known limitations

- Baselines are provisional; the composite is a research indicator, not a diagnosis.
- Time-coded AOI windows align to the AVPlayer media clock; the first ~100 ms of a clip falls
  back to wall-clock before the first periodic-observer tick.
- stim_06 (gears) is in the baseline set but not part of either shipped playlist.
- Loop wraps reset fixation segmentation per monotonic run (by design); dwell ratios are
  loop-agnostic (count-based).
- `first_entry_latency_ms` measures entry, not a full saccadic-latency estimate; a proper
  saccade-locked latency needs event-tagged onsets (D1.4 tagging is still open).
