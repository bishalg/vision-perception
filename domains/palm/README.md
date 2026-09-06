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
