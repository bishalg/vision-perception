# Palm Astrology Prediction — Status, Architecture & Phased Roadmap

**Date:** 2026-09-13
**Status:** Draft for review — written for an AI agent (or engineer) to pick up and track against. Every claim below was verified against the current tree (file paths + line-level evidence), not inferred from doc comments. Where a cited doc is aspirational rather than built, this doc says so explicitly.
**Relationship to other docs (do not duplicate, cross-reference):**
- [`product_vision.md`](product_vision.md) — north star: "the user buys a prediction, never a detection."
- [`palm_robust_roadmap.md`](palm_robust_roadmap.md) — the CV/ML **detection-quality** track (Steger → hybrid ML linker). This doc assumes that track is in progress and does not restate it; see §4 for the boundary between the two docs.
- [`palm_fate_line_spec.md`](palm_fate_line_spec.md) — the only line that currently has a real detection→meaning mapping. This doc generalizes that pattern to the other three lines (§6–§8).
- [`palm_region_overlay_spec.md`](palm_region_overlay_spec.md) — live-camera mount/corridor overlay (Swift-only today; `PalmRegionMap` C++ is still just a comment, see §5).
- [`research/palm_next_phase.md`](research/palm_next_phase.md) — its P0 wiring is largely done in `PalmDomainPack.cpp` (`extractMajorLines`, chained `major_lines` objects, per-line `gap_count`; the old `continuity_breaks` key is retained for compatibility); do not re-plan those items.

**How to use this doc:** each track in §8 has a status, owner files, and a definition of done. An agent picking up a track should update its status inline (`NOT STARTED` → `IN PROGRESS` → `DONE`, with a one-line note + commit hash) rather than opening a parallel tracking doc.

---

## 0. TL;DR status board

| Layer | Status | Where |
|---|---|---|
| Crease detection (Steger → chained splines → geometry metrics) | **WORKING** | `core/src/ridge/`, `domains/palm/src/CreaseChainLinker.cpp`, `CreaseGeometry.cpp` |
| Hand anatomy decomposition (digit rays, palm disc) | **WORKING** | `domains/palm/src/HandAnatomyDecomposer.cpp` |
| Mount zones (7 polygons, membership only) | **WORKING**, but zone-membership only — no size/prominence grading | `domains/palm/src/PalmZones.cpp` |
| Fate-line detection→meaning engine | **WORKING** (19/30 classical cases implemented) | `domains/palm/src/FateLineAnalyzer.cpp`, `FateLineCases.hpp` |
| Heart/Head/Life detection→meaning engine | **WORKING** (structural P0 cases: presence, short/long, broken, merges) | `HeartHeadAnalyzer.cpp`, `LifeLineAnalyzer.cpp`, `LineCaseTable.hpp` |
| Short/long line classification | **All 4 lines**, placeholder ratios, `calibrated:false` | `LineLengthThresholds.hpp`, `major_lines[*].is_short/is_long/length_basis` |
| Age-along-line mapping | **Fate + life** (life: linear 0..90 + optional head anchor, `age_calibrated:false`) | `FateLineAnalyzer.cpp` `ageAtY()`, `LifeLineAnalyzer.cpp` `ageAtLifeArc()` |
| Mount prominence (grill/star/cross marks) | **WORKING** (Cross/Star centroids + Island for mercury + junction-density grill proxy + horiz-fragment proxy) | `MountMarkDetector.cpp`, `FateReport.bad_saturn/mercury_mount` |
| Mobile reading text (`SimplePalmReading`) | **Report-augmented** — legacy prose + measured clauses from all four reports; legacy output byte-identical when reports absent | `SimplePalmReading.swift/.kt`, `PalmAnalysisResult.swift` / `PalmAnalysisResult.kt` |
| VLM/LLM reading generation | **Prompt-construction only**, desktop CLI, no live API call, not wired to mobile | `domains/palm/src/VlmPromptFormatter.cpp`, `tools/lib/curv/tools/hand_inspector_lib.cpp` |
| Palm ML training pipeline | **Does not exist** | — |
| Dataset | 52 samples (36/10/6 split), geometry-only labels, **zero prediction/astrology labels** | `data/samples/mobile_captures/manifest.json`, `data/palm_labels/` |
| "Phase 4" (prediction) naming | **Never defined anywhere in the repo** — this doc defines it (§2) | — |

---

## 1. Current end-to-end architecture (as-is)

```
Capture (iOS/Android)
  → 21 MediaPipe landmarks + RoiWarper → 512×512 canonical palm patch
  → HandAnatomyDecomposer (digit rays, basal MCP crease, palm-disc polygon)   [WORKING]
  → StegerRidgeExtractor (sub-pixel ridge/valley segments)                   [WORKING, core/, domain-agnostic]
  → CreaseChainLinker (chain segments → 4 named ChainedCreaseLine splines,   [WORKING]
      filtered by palm-disc membership)
  → CreaseGeometry (arc length, chord length, tortuosity, mean angle,        [WORKING]
      fork/gap counts, intersections, zone membership)
  → PalmZones (7 mount polygons, membership tagging only)                   [WORKING, membership-only]
  → FateLineAnalyzer → FateLineCases table → FateReport                     [WORKING, fate only]
      (is_short/is_long, origin, termination, age_at_end, marks — partial)
  → PalmDomainPack assembles domain_payload JSON                           [WORKING]
        { major_lines: {...}, fate_analysis: {...}, zone_distribution, ... }
  → [DESKTOP ONLY] VlmPromptFormatter → Gemini-shaped prompt JSON            [WORKING, prompt-only, no HTTP call]
  → [MOBILE] PalmAnalysisResponse decodes domain_payload
        ⚠ PalmDomainPayload has no `fateAnalysis` field → dropped silently
  → SimplePalmReading: 4 geometry scalars (confidence/length/points/tortuosity)
        → hard-coded English sentence templates, per line                   [STUB — does not use FateReport at all]
  → SamudrikaReadingSheet renders that text as the "Phase 3" reading screen
```

**The gap this whole doc is about:** the pipeline has a real, working, classically-grounded detection→meaning engine (`FateLineAnalyzer`), but only for one of four lines, and even that one engine's output never reaches the screen the user actually reads. Heart/Head/Life get no classical reasoning at all — just generic length/confidence prose.

---

## 2. Naming the missing phase

The mobile app already uses `Phase 1/2/3` in code comments (`MainScannerView.swift`, `CaptureResultView.swift`, `CreaseLineEditorView.swift`, `SamudrikaReadingSheet.swift`):

- **Phase 1** — live capture (camera + guidance HUD).
- **Phase 2 / 2b** — crease line verification + manual correction (`CreaseLineEditorView`).
- **Phase 3** — reading presentation (`SamudrikaReadingSheet`) — currently fed by the `SimplePalmReading` stub.

There is no Phase 4 anywhere in the palm code (every "Phase 4" hit elsewhere in the repo belongs to the unrelated flood/geospatial domain). This doc defines it:

- **Phase 4 — Prediction Synthesis.** The step between "lines are geometrically correct" (end of Phase 2) and "reading is shown" (Phase 3): running all four lines' case-table matches + mount grading + cross-line negation rules into one unified reading object. This is the direct answer to *"what's next after the lines are all corrected and drawn"* — see §8, Track 6–7.

---

## 3. Data collection status

- **52 total samples** (`data/samples/mobile_captures/manifest.json`, schema `curvstudio_dataset_split_v1`): 36 train / 10 valid / 6 test. 50 are Roboflow-sourced stock hand photos (`data/palm_labels/raw/rf_000NN_*.jpg`); 2 are real phone captures (`01_left_bishal`, `02_right_bishal`).
- **Label format is geometry-only.** `*_USER_CORRECTED*.json` = `{"heart": [[[x,y],...]], "head": [...], "life": [...], "fate": [...]}`, normalized polyline points, ~50 points/line. **No astrology/meaning/age label exists in any sample.** This is a corrected *shape* label for retraining the detector (per `product_vision.md`'s edit-to-learn loop), not a prediction-training label.
- **`evidence.json` fixtures are stale relative to code.** `01_left_bishal_evidence.json` has no `domain_payload.fate_analysis` key at all — it predates the `FateLineAnalyzer` wiring. Regenerate fixtures (`hand_inspector_cli`) before using them as reference/regression data for Phase 4 work.
- **Gate before any ML training** (per `STUDIO_GUIDE.md`): don't train until ≥50–100 `reviewed: true` labels exist. We're at 52 total, review status per-sample not audited here — check before starting Track 1 of `palm_robust_roadmap.md`.
- **No prediction/reading dataset exists at all.** Even once detection is solid, there is nothing to *evaluate* a reading against — see §9's open decision on this.

---

## 4. Boundary with the CV-robustness track

`palm_robust_roadmap.md` owns making the *geometry* (arc length, chain identity, confidence) trustworthy — that's a prerequisite for everything below but is tracked separately. This doc assumes: whatever geometry `CreaseChainLinker`/`CreaseGeometry` (or its eventual ML successor) outputs, in whatever confidence range, needs to be turned into a reading. **Prediction-layer work (this doc) does not need to wait for CV-robustness work to finish** — it needs the *shape* of the data (which fields exist), not their final accuracy, so Tracks 2–7 below can and should run in parallel with the CV track, per the user's explicit ask.

---

## 5. How a line currently maps to an astrology prediction (the only working example)

`FateLineAnalyzer.cpp` + `FateLineCases.hpp` is the reference pattern — study this before building the other three.

1. **`FateLineCases.hpp`** — a flat table of ~30 `FateCaseDef` entries, each: `{id, group (Presence/Origin/Course/Termination/Marks/Age/Context), blog_anchor (classical source citation), primitive (the CV rule, as text), use (the palmistry meaning, as text), priority (P0–P3), implemented (bool)}`. This *is* the "line feature → astrology meaning" mapping table the user is asking about — it just only exists for fate.
2. **`FateLineAnalyzer.cpp`** implements the `primitive` rules from the table above: computes `is_short`/`is_long` (fixed ratio thresholds vs. palm height, §6), `origin`/`termination` (coordinate-band tests), `age_at_end`/`age_at_break` (§7), and cross-references heart/head/life for negation (e.g. "short fate line, but negatable by strong Venus/Mercury"). Output: a `FateReport` struct.
3. **`PalmDomainPack.cpp`** puts `FateReport` into `domain_payload.fate_analysis`.
4. **Dead end today:** `PalmDomainPayload` (Swift) never declares a `fateAnalysis` field, so `JSONDecoder` silently drops it. `SimplePalmReading.swift` never sees it. The richest reasoning in the whole codebase currently never reaches a user. **This is Track 6 (§8) — a small, high-leverage fix.**
5. `VlmPromptFormatter.cpp` *does* format the full `major_lines` + cross-line structure into a grounded Gemini-style prompt (with an explicit anti-hallucination system instruction — "if a feature has zero verified segments, declare it unverified, don't hallucinate"), but only `hand_inspector_cli` (desktop) calls it, and even there it only writes `gemini_prompt.json` to disk — no live API call exists anywhere in the palm code.

---

## 6. Short vs. long line — generalized classification design

**Current state:** only `fate_line` has this, as fixed ratio constants normalized against palm height (`patch_dim`, the 512-canonical space):

```cpp
// domains/palm/src/FateLineAnalyzer.cpp:13-16
constexpr float kPresentMinLength = 40.0F;
constexpr float kPresentMinConf   = 0.55F;
constexpr float kShortRatio = 0.40F;   // arc < 0.40 * palm_height → "short"
constexpr float kLongRatio  = 0.70F;   // arc > 0.70 * palm_height → "long"
```

Separately, and **disconnected from the above**, `SimplePalmReading.swift:68-73` buckets length in *absolute pixels* (85/130/180px) purely for its own prose — a different scheme, not normalized against hand size at all, so it drifts with camera distance/zoom.

**Why one pair of constants doesn't generalize:** the user's instinct is correct — classical Samudrika Shastra defines "short" and "long" differently per line, because each line's natural anatomical span is different (fate runs wrist→fingers roughly vertically; heart runs pinky-side→index-side roughly horizontally under the fingers; head runs a similar band lower; life arcs tightly around the thenar mount). A single 0.40/0.70 ratio pair is already fate-specific, not universal — it just hasn't been made *explicit* as being fate-specific because nothing else exists yet to contrast it with.

**Design — `LineLengthThresholds` (new):**

- New file: `domains/palm/include/curv/domain/LineLengthThresholds.hpp` — same pattern as `FateLineCases.hpp`, one entry per line:

  ```cpp
  struct LineLengthThresholds {
      std::string line_id;       // "heart_line" | "head_line" | "life_line" | "fate_line"
      float short_ratio;         // arc_length_px / normalization_basis <= this → "short"
      float long_ratio;          // >= this → "long"          (between = "medium")
      std::string basis;         // what it's normalized against (see below)
      std::string classical_note;// citation / rationale, mirroring blog_anchor in FateLineCases
      bool calibrated;           // false until data-calibrated (see below) — start false for all
  };
  ```

- **Normalization basis must stay consistent with the fate-line precedent** (palm height in the 512-canonical patch) for lines that run roughly vertically (fate), but **heart and head lines run roughly horizontally** — normalizing them against *palm height* would be anatomically wrong. Use `HandAnatomyDecomposer`'s palm-disc bounding geometry to pick the correct basis per line orientation:
  - `fate_line`, `life_line`: normalize against palm **height** (existing convention).
  - `heart_line`, `head_line`: normalize against palm **width** (pinky-mount to Jupiter-mount span) — a new basis to compute, likely already derivable from `PalmZones` mount polygon extents; check before adding new geometry.
- **Starting constants (placeholders, not yet calibrated — flag this to the user in any reading text until calibrated):**

  | line | short_ratio | long_ratio | basis | rationale |
  |---|---|---|---|---|
  | fate_line | 0.40 | 0.70 | palm height | existing, keep as-is |
  | life_line | 0.40 | 0.70 | palm height | mirrors fate's span; **flag in reading copy** that "short life line" is classically *not* a lifespan claim — avoid literal, alarming phrasing |
  | heart_line | 0.55 | 0.80 | palm width | classical distinction: ends under index (short) vs. crosses to Jupiter mount edge (long) |
  | head_line | 0.45 | 0.75 | palm width | classical distinction: ends below middle finger (short) vs. reaches Mercury-mount edge (long) |

- **Calibration plan:** these four rows are *placeholders*, in the same unvalidated state fate's constants are already in today (hand-picked, not data-derived). Once the dataset passes the ≥50–100 reviewed-label gate (§3), add a calibration script (`tools/calibrate_line_thresholds.py` — does not exist yet) that computes short/long as dataset percentiles (e.g. 25th/75th) per line instead of guessed constants, and back-fills `calibrated: true`.
- **Status: DOES NOT EXIST for heart/head/life.** This is Track 2 (§8).

---

## 7. Age-along-line mapping — scope and design

**Current state:** fate line only. `FateLineAnalyzer.cpp`'s `ageAtY()` anchors a normalized-y → age mapping using the fate line's intersection points with the head line (~age 35) and heart line (~age 53), falling back to a linear 20–70 span when an anchor is missing. Used to compute `age_at_end` and `age_at_break` (for interrupted fate lines).

**Do not generalize this to all four lines.** In classical Samudrika Shastra, age-timeline reading is conventionally a **fate-line and life-line** concept — heart and head lines are read for *quality* (temperament, intellect) not *chronology*. Forcing an age mapping onto heart/head would be inventing a classical claim with no source, which conflicts with `VlmPromptFormatter`'s explicit anti-hallucination stance. Scope age-mapping work to **{fate: done, life: to do}** only.

**Design for life-line age mapping** (structurally a copy of the proven `FateLineAnalyzer::ageAtY()` pattern):

- New: `domains/palm/include/curv/domain/LifeLineAnalyzer.hpp` / `.cpp`.
- Anchor points: life-line intersections with head line and heart line (the `intersections` field already computed in `CreaseGeometry`/present in `evidence.json` — reuse, don't recompute).
- Classical age range convention: typically 0 (birth) near the index-thumb web / top of the life-line arc, to ~85–100 at the wrist end — verify this specific mapping against a cited classical source (mirror how `palm_fate_line_spec.md` cited `palmistindian.com`) before hard-coding numbers; do not invent the citation.
- Output: fold into a new `LifeReport` (see Track 4, §8) analogous to `FateReport`.
- **Status: DOES NOT EXIST.** This is Track 3 (§8) — low-risk since it's a structural port of a pattern that's already implemented and tested.

---

## 8. Phased roadmap (parallel tracks)

Tracks 2–5 have no hard dependency on each other or on the CV-robustness track finishing (§4) — they can start now, in parallel, on today's geometry output. Track 6 depends on at least one of 2–5 existing to have something worth wiring. Track 7 depends on 6. Track 1 (CV robustness) is tracked in `palm_robust_roadmap.md`, referenced here only for sequencing context.

| # | Track | Status | Depends on | Owner files (new unless noted) | Definition of done |
|---|---|---|---|---|---|
| 1 | CV/ML detection robustness | IN PROGRESS (see `palm_robust_roadmap.md`) | — | *(tracked in that doc)* | *(tracked in that doc)* |
| 2 | Short/long classification for heart/head/life (§6) | DONE 2026-09-13 (uncommitted) — `LineLengthThresholds.hpp`, per-line `is_short`/`is_long`/`length_basis` in `major_lines[*]`, `test_line_length_thresholds` (5 tests) | none | `LineLengthThresholds.hpp` (new), `PalmDomainPack.cpp` (wire `is_short`/`is_long` per line, mirroring fate's fields) | All 4 lines emit `is_short`/`is_long`/`basis` in `domain_payload.major_lines[*]`; unit test mirroring `test_fate_line_analyzer.cpp` |
| 3 | Life-line age mapping (§7) | DONE 2026-09-13 (uncommitted) — `LifeLineAnalyzer` (linear 0..90 + optional head anchor, `age_calibrated:false`), `test_palm_line_analyzers` | none | `LifeLineAnalyzer.hpp/.cpp` (new), `test_life_line_analyzer.cpp` (new — folded into `test_palm_line_analyzers.cpp`) | `LifeReport.age_at_*` populated; classical age-range source cited in code comment |
| 4 | Heart/Head/Life case tables (mirror `FateLineCases.hpp`) | DONE 2026-09-13 (uncommitted) — generic `LineCaseDef` tables (6+6+5 rows, structural P0s implemented, unresearched rows carry RESEARCH NEEDED), `HeartReport`/`HeadReport` + analyzers, tested | benefits from 2 & 3 but can scaffold now with `implemented:false` rows | `HeartLineCases.hpp`, `HeadLineCases.hpp`, `LifeLineCases.hpp` (new — implemented as `LineCaseTable.hpp` tables) + matching `*Analyzer.cpp` (new — `HeartHeadAnalyzer.cpp`) + 3 new spec docs mirroring `palm_fate_line_spec.md` (research classical sources per line — **do not invent citations**, this needs a research pass like the fate-line spec had) | Each line has a `{Line}Report` struct + case table with ≥1 P0 case each, tested |
| 5 | Mount prominence (grill/star/cross marks) | DONE 2026-09-13 (uncommitted) — `MountMarkDetector` (Cross/Star centroids + Island for mercury + junction-density grill proxy + horiz-fragment proxy), both rows flipped to `implemented:true`, flags on `FateReport` | none | `PalmZones.cpp`/new `MountMarkDetector.hpp/.cpp` | At least the two already-specified mount-mark cases flip to `implemented:true` and are used by `FateLineAnalyzer`'s existing negation logic |
| 6 | Wire reports into mobile (fix the silent-drop bug) | DONE 2026-09-13 (uncommitted) — `fate/heart/head/life_analysis` decode on iOS + Android, report-measured clauses in both readings, call sites pass reports, fixtures regenerated with all four analyses | 2–5 make this worth doing incrementally; can start today just for fate | `PalmAnalysisResult.swift` (add `fateAnalysis`/future report fields to `PalmDomainPayload`), `SimplePalmReading.swift` (replace/augment scalar-template prose with case-table-driven text), same on Android (`SimplePalmReading.kt` per `mobile_parity.md`) | Fate reading on-screen visibly reflects `FateReport` fields (short/long, origin, age, marks) instead of only generic length/confidence prose |
| 7 | Real prediction synthesis (Phase 4, §2) | NOT STARTED — blocked on §9 human decisions (VLM relay/infra + reading-correctness ground truth) | 6 | `VlmPromptFormatter.cpp` (extend to all 4 lines' reports), new networking client (mobile or a thin server relay — **product decision needed**, see §9), `SamudrikaReadingSheet.swift` (render Phase-4 output) | Decide rules-only-local vs. VLM-grounded (§9); whichever is chosen, a single unified reading object (all 4 lines + mounts + cross-line rules) renders in Phase 3's screen, replacing per-line-only templates |
| 8 | Retrain/calibration flywheel | NOT STARTED, blocked on data volume | 1, 3 (needs `USER_CORRECTED` volume + Track 2's calibration script hook) | `tools/calibrate_line_thresholds.py` (new) | Threshold constants in §6 flip from placeholder to `calibrated: true` once ≥50–100 reviewed samples exist |

**Direct answer to "what's next after the lines are all corrected and drawn":** Track 7 (Phase 4 — Prediction Synthesis). Concretely: once Phase 2's corrected geometry exists, run Tracks 2–5's analyzers on it, assemble one unified reading object, and render it in Phase 3 (Track 6 wires the currently-orphaned `FateReport` in as a first, low-effort proof of this exact flow).

---

## 9. Open decisions that need a human/product call (not agent-autonomous)

- **No ground truth exists for "reading correctness."** Geometry has a clear ground truth (a human-corrected polyline). A reading ("your fate line suggests career instability in your 30s") has no equivalent — there's no dataset or metric to check a prediction against. This needs a product decision (e.g., user feedback capture — "did this feel accurate?" — as a new, currently-nonexistent data source) before Track 7 can claim to be "working" in any measurable sense, as opposed to merely "producing plausible-sounding text."
- **VLM live integration (Track 7):** requires an API-key/infra decision (which provider, where the call originates — on-device isn't realistic for a hosted LLM, so this likely needs a thin backend relay that doesn't exist yet) and a cost/latency tradeoff analysis. Do not default to "just call Gemini from the app" without that decision being made explicitly.
- **Threshold placeholders (§6) and the life-line age range (§7)** are not yet calibrated/cited. Any reading text produced before calibration should be treated as a development scaffold, not shipped as an authoritative claim to end users.
- **`evidence.json` fixtures regenerated 2026-09-13** (`hand_inspector_cli`): both mobile-capture fixtures now carry all four analyses. Regenerate again after any analyzer change before using them as reference data.
