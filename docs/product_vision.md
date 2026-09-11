# Product Vision & Core Design Principles

> **One-line goal:** the user buys a **prediction / result**, never a detection.
> CV lines are plumbing. Palm lines exist to produce a **Samudrika reading**;
> river lines exist to produce a **flood alert + forecast**. If a screen shows
> confidences instead of a result, the product has failed.

---

## 1. End goal per project

| Project | Detection (means) | Prediction (product) | User-facing result |
| :--- | :--- | :--- | :--- |
| Palm apps (`apps/ios_palm_app`, `apps/android_palm_app`) | Steger ridges → Heart/Head/Life/Fate polylines (`domains/palm`) | Rule-based `SimplePalmReading` → grounded VLM report | Reading card: per-line interpretation + metrics; never raw CV |
| Flood + CCTV (`domains/geospatial`, `adapters/cctv`, `apps/cesium_viewer`) | Steger ridges → river channels, width proxy, pier waterline | `flood_delta.json` → Bedrock `getFloodEvidence → dispatchFloodAlert` + twin banner | Alert + forecast + 3D twin; never ridge counts |

Every detection change must move a **prediction metric** (report accuracy,
alert precision, width repeatability) — otherwise it does not ship.

## 2. Human-in-the-loop (non-negotiable)

- **Humans detect better than the model today.** That is expected, not a bug.
- The app is a **helper that makes human work faster**: it proposes lines,
  the human corrects them with cheap edits (drag/add/remove/relabel), then
  confirms with **Use these lines for reading**.
- The confirmed, human-edited geometry — not the raw auto output — is what
  the prediction layer consumes (`predictionReadySplines` /
  `predictionPayloadJSON()` on iOS, equivalents on Android).
- Full automation is desirable **at the end**, not the gate: auto-proposals
  improve gradually, but the app must be fully usable with human correction
  from day one. Absent/faint is a valid result (`no_fate`, `no_segments`),
  never a dead end.

## 3. Edit-to-learn loop (how we get to automatic)

Each confirmed capture is training data for the next iteration:

```text
capture → auto propose → human edit (fast) → confirm
    → store {image, auto polyline, human-corrected polyline, report}
    → retrain / retune scorer → better proposals next time
```

- Ground truth lives in `data/samples/mobile_captures/*_USER_CORRECTED.*`
  (today) and `data/palm_labels/` (at scale).
- Progress is measured on prediction-grounded metrics (`IoU` vs human
  polyline, report agreement), tracked in `docs/palm_crease_baseline.md`
  and `docs/PROGRESS.md` — not on CV internals alone.

## 4. Architecture consequences

- `core/` stays domain-agnostic (see `docs/architecture.md:13`): no palm,
  flood, or prediction vocabulary inside it.
- `domains/` own detection + prediction shaping; `apps/` own human editing
  + result presentation. The overlay kit (`apps/mobile_overlay`) **never
  decides** — it guides capture and renders proposals.
- Mobile packaging (`apps/ios_palm_app`, `apps/android_palm_app`) and flood
  cloud/twin work proceed in parallel because they share only `core/`.
