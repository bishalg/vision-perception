# Palm Domain Pack (`org.curv.domain.palm`)

Interprets extracted sub-pixel ridges as anatomical palm creases over the
canonical 512x512 upright palm patch. Implements `IDomainPack`; strictly no
palm logic ever enters `core/`.

## Units

- **`PalmDomainPack`** (`include/curv/domains/PalmDomainPack.hpp`) — maps
  `RidgeGraph` segments onto anatomical zones, scores continuity/branching,
  emits domain `Evidence` + a `domain_payload` JSON object for the evidence report.
  Handles omnidirectional continuity breaks (segments are joined regardless of
  stored direction).
- **`PalmZones`** (`include/curv/domain/PalmZones.hpp`) — canonical 512x512
  anatomical zone polygons (e.g. `jupiter_zone`, `saturn_zone`, ...), with
  left/right handedness mirroring (`getCanonicalPalmZones(dim, is_left)`).
- **`PalmParameters`** (`include/curv/domain/PalmParameters.hpp`) —
  palm-radius-scaled `StegerConfig` (`computeFromLandmarks`) so extraction
  adapts to hand size/distance.
- **`VlmPromptFormatter`** (`include/curv/domain/VlmPromptFormatter.hpp`) —
  builds grounded, anti-hallucination prompts for Gemini/Gemma from the evidence
  JSON (classical Vedic Samudrika Shastra grounding);
  `formatGeminiRequest(evidence, question)` produces a ready-to-POST Gemini API
  payload. Prompt-only by design — the network call belongs to the caller.

Handedness is detected in `core` (`geometry::detectIsLeftHand`) from the 21
landmarks and propagated via frame metadata; the C-ABI landmark path wires it
all together end-to-end.

## End goal: prediction, not lines

Lines are the means; the product is the **Samudrika reading**. The domain emits
`major_lines` + `fate_analysis`, the app lets the human correct them fast, and
only confirmed geometry feeds the report (`SimplePalmReading` → grounded VLM).
Absent/faint (`no_fate`) is valid. See [`docs/product_vision.md`](../../docs/product_vision.md).

## Human-in-the-loop + learning loop

Human corrections are training data: every confirmed capture (`*_USER_CORRECTED.*`
in `data/samples/mobile_captures/`, later `data/palm_labels/`) retunes the
scorer, so proposals improve gradually toward full automation.

## Next phase

Extraction (Steger + warp + mask + chaining) is in place. The next product work
is **robust result analysis**, not a new detector. See
[`docs/research/palm_next_phase.md`](../../docs/research/palm_next_phase.md):
wire `CreaseChainLinker` as the named-line source of truth, emit geometric
`palm_analysis` / `user_report`, and stop sending hundreds of raw ridge
candidates to the VLM.
