# Palm App Next Phase: Robust Result Analysis (Do Not Rebuild the Detector)

> **Identifier**: `DOC-RES-006`
> **Status**: Planning — next palm product work
> **Date**: 2026-09-09
> **Audience**: Engineering + product. Use this as the build checklist, not as a research essay.

---

## 1. Decision (read this first)

The OpenCV / C++ **detector is not the bottleneck for the next user-facing release**.

The next phase is **result analysis**: turn the already-extracted ridge graph into a small, trusted reading that a person (and a VLM) can use.

| Keep (do not reinvent) | Build next |
| :--- | :--- |
| Steger sub-pixel ridge extraction (`core/ridge`) | Geometric features per named crease (length, curvature, angle, forks) |
| 21-point landmark warp (`HandLandmarkDetector` + `RoiWarper`) | Line **identity** with confidence, not a single lucky segment ID |
| Lab \(a^*\) hand mask (`HandBoundaryDetector`) | User report JSON + overlay that only draws verified lines |
| Catmull-Rom chaining (`CreaseChainLinker`) | Grounded analysis prompt that **stops dumping 400 raw candidates** |
| Quality gate + overlay renderer | Capture UX later (Phase C mobile). Analysis first on desktop CLI |

**Do not** replace Steger with U-Net or Canny. **Do not** vendor hobby palmistry GitHub apps.

---

## 2. What already exists (reuse this)

Pipeline as of 2026-09-09 (`hand_inspector_cli`):

```text
photo
  → 21 landmarks (ONNX / MediaPipe topology, heuristic fallback)
  → canonical 512×512 upright palm (`RoiWarper`)
  → Lab a* hand mask + ridge prune (`HandBoundaryDetector`)
  → digit rays + palm disc (`HandAnatomyDecomposer`)
  → Steger ridges (`StegerRidgeExtractor`)
  → zone map (`PalmZones` + `PalmDomainPack`)
  → Catmull-Rom major-line chains (`CreaseChainLinker`)
  → overlay PNG + evidence JSON + Gemini prompt (`VlmPromptFormatter`)
```

That stack is the product engine. GitHub palmistry apps do **not** have an equivalent classical topology. Next work hangs **off** this graph. It does not replace it.

| Unit | Path | Role next phase |
| :--- | :--- | :--- |
| Steger extractor | `core/src/ridge/StegerRidgeExtractor.cpp` | Source of centerlines, width, intensity. Frozen API. |
| Landmark + warp | `adapters/opencv_dnn/`, `core/.../RoiWarper` | View-invariant palm (same idea as yeonsumia MediaPipe warp). |
| Hand mask | `adapters/opencv/HandBoundaryDetector` | Drop table grain / background ridges. |
| Anatomy | `domains/palm/HandAnatomyDecomposer` | Restrict creases to palm disc, not fingers. |
| Zone pack | `PalmDomainPack`, `PalmZones` | Mount counts. Must stop being the only line-namer. |
| Linker | `CreaseChainLinker` | Must become the **canonical** Heart/Head/Life/Fate object. |
| Overlay | `adapters/opencv/OverlayRenderer` | Draw chained splines + labels, hide junk ridges in user mode. |
| Prompt | `VlmPromptFormatter` | Consume **analysis JSON**, not 450 `line_candidate` rows. |
| C-ABI / JNI / Swift | `adapters/capi`, `android`, `apple` | Same evidence contract on phone later. No second engine. |

Architecture rule remains: **no palm vocabulary in `core/`**. Features and Samudrika labels live in `domains/palm/` only.

---

## 3. GitHub landscape — borrow ideas, not code

Audited 2026-09-09. Full component policy: [`github_reuse_matrix.md`](github_reuse_matrix.md) and [`docs/license_policy.md`](../license_policy.md).

| Repo | License | What they do | Verdict |
| :--- | :--- | :--- | :--- |
| [lakshay102/Palm-Astro-Application](https://github.com/lakshay102/Palm-Astro-Application) | Unlicensed (class project) | U-Net + ResNet18, 4-class mask (bg / life / head / heart). Dummy data, 10-epoch train, Gradio demo. | **AVOID code.** Steal the **feature list** only (see §5). |
| [yeonsumia/palmistry](https://github.com/yeonsumia/palmistry) | Apache-2.0 | MediaPipe warp → trained U-Net → K-means to name lines → length vs landmark thresholds. | **Do not vendor.** Warp is already here. K-means-on-pixels is weaker than chaining Steger polylines. |
| Typical OpenCV tutorials | n/a | `cv2.Canny` + morphology | **Forbidden** on the crease path. Canny traces crease *shadows* as double edges and picks up table grain. |

Two buckets we deliberately stay out of:

1. **Deep-learning-heavy apps** — need labeled masks, GPU training, and still output pixel blobs, not sub-pixel centerlines.
2. **Naive classical CV** — Canny / Hough cannot model curved valleys.

The real literature to **read** (not copy GitHub notebooks) is academic **palmprint principal-line extraction** (Gabor, finite Radon, ridge filters on CASIA / PolyU). Use those papers as measurement ideas. Do not pull GPL research code into `core/`.

### Optional later (not this phase)

A tiny ONNX **labeler** that classifies already-extracted polylines as Heart / Head / Life / Fate / other. Weights only, Apache-2.0 / MIT, run via existing `cv::dnn` adapter. Never a U-Net that *finds* the lines.

---

## 4. Honest gap: current user analysis is not robust yet

Desktop extraction works. The **report** does not.

From `data/samples/left_prompt.json` (real run):

| Signal | Value | Why it hurts the user |
| :--- | :--- | :--- |
| Segments analyzed | 450 | Most are pores, wrinkles, finger skin — not principal lines |
| Continuity breaks | 788 | Pairwise gap count on *all* fragments; not a palmistry feature |
| Junctions | 0 | Topology not wired into the named lines |
| Zone `unknown` | 198 | Almost half the ridges are unlabeled |
| Prompt body | hundreds of `line_candidate` rows | VLM cannot tell signal from noise; halluciates from clutter |
| Line IDs | single `heart_line_id` / `head_line_id` | One fragment, not the chained spline the overlay already draws |

Two namers currently disagree:

- `PalmDomainPack` — zone + angle heuristics, one segment per major line.
- `CreaseChainLinker::extractMajorLines` — chained spline used by the overlay.

**Next phase rule:** the chained spline is the source of truth. `domain_payload` must describe **that** object, with geometry and confidence. Overlay colors and the VLM prompt must read the same JSON.

---

## 5. Target contract: what the user (and VLM) receive

One small JSON object, `palm_analysis`, nested under `domain_payload`. Core evidence schema stays domain-agnostic ([`docs/evidence_schema.md`](../evidence_schema.md)).

### 5.1 Capture / quality (already mostly present)

Reuse Laplacian / brightness / contrast. Add palm-specific gates:

- `hand_mask_valid`, `palm_disc_valid`, `landmarks_source` (`onnx` | `heuristic`)
- `analysis_ready` — false if blur, no mask, or no major line above confidence floor

If `analysis_ready` is false, **do not invent a reading**. Return reasons and a recapture hint.

### 5.2 Per-line geometry (steal this list from Palm-Astro; compute it on Steger polylines)

For each of `heart_line`, `head_line`, `life_line`, `fate_line`:

| Field | Meaning | How (reuse) |
| :--- | :--- | :--- |
| `present` | Verified vs absent | Confidence ≥ threshold after chaining |
| `confidence` | 0–1 | Length + zone agreement + mask coverage + chain fragment count |
| `segment_ids` | Constituent Steger IDs | `ChainedCreaseLine.constituent_segment_ids` |
| `arc_length_px` | Polyline length | Already on `ChainedCreaseLine.total_length` |
| `chord_length_px` | Endpoint distance | Two points |
| `tortuosity` | `arc / chord` | Palm-Astro “curvature” proxy; cheap and stable |
| `mean_angle_deg` | Orientation vs horizontal | From chained points |
| `start_zone` / `end_zone` | Mount / region | `ZoneIntersector` on endpoints |
| `fork_count` | Branches off the chain | Junctions of constituent segments |
| `gap_count` | Bridged interruptions | Linker gaps, not global 788 |
| `mean_width_px` | Crease thickness | Steger width samples on those segments |
| `mean_intensity` | Dark-ridge strength | Already on chained line |

Plus a short `notes[]` of geometric facts only, e.g. `"life_line_encircles_thenar"`, `"head_heart_separated"`. No fortune text here.

### 5.3 Cross-line structure

- `heart_head_min_distance_px` — parallel vs joined (classic palmistry fork)
- `named_line_intersections` — only among the four chains, not all 450 segments
- `dominant_line` — longest verified arc (Palm-Astro idea; keep it geometric)
- `palm_type_proxy` — mean tortuosity of verified majors: `straight` / `balanced` / `curved` (thresholds documented, not mystical)

### 5.4 User-visible report (deterministic, before any LLM)

A second object `user_report` generated **only** from `palm_analysis`:

```json
{
  "verdict": "complete" | "partial" | "insufficient",
  "lines": {
    "heart_line": { "status": "verified", "summary": "Transverse crease, 182 px, low tortuosity." },
    "fate_line": { "status": "unverified", "summary": "Not confirmed in this capture." }
  },
  "caveats": ["heuristic_landmarks", "partial_mask"],
  "disclaimer": "Geometric description only. Not medical, legal, or predictive advice."
}
```

Samudrika / interpretive prose stays **optional** and **downstream**: `VlmPromptFormatter` gets `palm_analysis` + `user_report` only. It must not receive the raw 450-candidate dump.

Prompt rule (replace current behavior):

1. Quality + analysis_ready
2. Four named lines with numbers
3. Cross-line distances / intersections
4. Unverified features listed as absent
5. User question

Cap evidence items in the prompt at the **named creases + a handful of high-confidence branches**, not every `line_candidate`.

---

## 6. Where the code goes

```text
domains/palm/
  CreaseGeometry.hpp/.cpp     NEW — tortuosity, angle, forks, pairwise distances
  PalmLineIdentity.hpp/.cpp   NEW — score Heart/Head/Life/Fate on chained candidates
  PalmAnalysisReport.hpp/.cpp NEW — palm_analysis + user_report JSON
  PalmDomainPack.cpp          WIRE — consume linker output; stop single-ID naming
  VlmPromptFormatter.cpp      SHRINK — prompt from palm_analysis only

tools/hand_inspector_lib.cpp  WIRE — same JSON to overlay labels + files
tests/unit/                   NEW — geometry + identity + prompt-size tests
```

`core/` unchanged except if a generic polyline metric (arc length, chord) is worth a tiny geometry helper. Prefer domain-local first.

C-ABI: `curv_pipeline_process_landmarks_roi` already returns JSON. When `PalmDomainPack` emits the new payload, JNI / Swift get it for free. No mobile rewrite this phase.

---

## 7. Phased build plan

### Phase P0 — Wire what we already built (days, not weeks)

Goal: one source of truth.

1. `PalmDomainPack::process` takes (or internally calls) `CreaseChainLinker::extractMajorLines`.
2. `domain_payload.major_lines` = chained objects, not lone IDs.
3. Overlay labels, evidence JSON, and VLM prompt all read `major_lines`.
4. Stop publishing `continuity_breaks` as a global pairwise count. Replace with per-line `gap_count`.
5. User overlay mode: draw chained majors + boundary + anatomy; hide unclassified ridges (keep a `--debug-all-ridges` flag).

Exit: `hand_inspector_cli` JSON has four line objects with `arc_length_px` and `segment_ids[]`. Prompt file is a few dozen lines, not hundreds of candidates.

### Phase P1 — Robust geometry + identity (this is the analysis engine)

1. `CreaseGeometry` metrics from §5.2.
2. Identity scorer: zone priors + orientation + palm-disc containment (`HandAnatomyDecomposer`) + mask coverage. Ambiguous → `present: false`, never a coin-flip label.
3. Cross-line min-distance and named intersections.
4. Confidence calibration on `data/samples/` left/right (and any new fixtures). Document failure cases: wet skin, rings, extreme tilt, heuristic landmarks.

Exit: unit tests for tortuosity/angle; left and right samples produce stable Heart/Head/Life when the overlay looks right; Fate may remain unverified.

### Phase P2 — User report + grounded VLM

1. `user_report` with `complete` / `partial` / `insufficient`.
2. Rewrite `VlmPromptFormatter` to the five-block prompt in §5.4.
3. Keep network I/O out of the library (existing rule).
4. Add a recapture string when `analysis_ready` is false (blur, no palm disc, no major line).

Exit: a human can read `user_report` without opening the overlay; Gemini payload contains **numbers**, not 450 identical `line_candidate` lines.

### Phase P3 — Product surfaces (after analysis is trusted)

Parallel, not a substitute for P0–P2:

- Desktop: `hand_inspector_cli` remains the golden path; optional simple GUI later.
- Mobile Phase C: Gradle / CameraX and SwiftPM / xcframework wrapping **the same C-ABI JSON**. Camera capture quality (focus, fill-the-frame, even light) matters more than a new detector.
- Optional ONNX line-labeler adapter **only if** P1 identity stays weak on a held-out photo set.

---

## 8. Explicit non-goals

- Do not implement U-Net / ResNet palm-line segmentation in this repo.
- Do not call `Canny`, `HoughLines`, or morphological skeletons as the crease detector.
- Do not copy source from lakshay102 (unlicensed) or yeonsumia (unnecessary; warp already exists).
- Do not put Samudrika / astrology strings in `core/` or in `CreaseGeometry`.
- Do not train a dataset of 200 hand photos unless P1 fails on real captures.
- Do not start Android/iOS packaging until `user_report` is stable on desktop samples.
- Do not send raw ridge graphs to the VLM.

---

## 9. Success criteria

| Check | Pass |
| :--- | :--- |
| Prompt size | Named lines + topology only; no full `line_candidate` dump |
| Overlay vs JSON | Same four chains, same labels |
| Absent lines | `unverified` / `present: false`, never a guessed story |
| Quality reject | `analysis_ready: false` + recapture hint, no reading |
| Identity | Heart / Head / Life agree with visual overlay on both `data/samples/left_*` and `right_*` |
| Architecture | New files only under `domains/palm/` (+ tests + inspector wiring) |
| Mobile | Zero new phone code required for P0–P2 |

---

## 10. Suggested GitHub / paper queries (research only)

Use these to **read** how others measure features. Do not import their detectors.

```text
"palm line extraction" OR "palm crease" AND opencv
"principal line extraction" AND palmprint
"MediaPipe" AND palm AND lines
Steger "curvilinear" crease OR palmprint
```

Papers worth a pass when calibrating P1: palmprint principal-line surveys (Gabor / Radon families). Compare **metrics**, not implementations.

---

## 11. Resume checklist

When starting implementation, do **P0** first:

1. Point `PalmDomainPack` at `CreaseChainLinker` output.
2. Shrink `VlmPromptFormatter` to `palm_analysis`.
3. Add `CreaseGeometry` tests.
4. Re-run `hand_inspector_cli` on `data/samples/` and diff overlay vs JSON.

The detector is done enough. The next win is a short, honest analysis object the user can trust.
