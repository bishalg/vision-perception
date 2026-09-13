# Shared mobile shape + theme specs (single source of truth)

These JSON files are the canonical geometry and theme tokens for the native
camera overlays. Both apps consume **these same files** at runtime — there are
no per-app, per-platform shape tables, so a fix here ships on iOS + Android:

| Artifact | Consumes |
| :--- | :--- |
| `silhouette.json` | `PalmSilhouetteView.swift` / `PalmSilhouette.kt` (tutorial hand) |
| `vastu_mandala.json` | `VastuMandalaView.swift` / `VastuMandala.kt` (3×3 grid identity) |
| `overlay_theme.json` | `OverlayPalette` (iOS) / `hud` `OverlayEmerald/Amber` (Android) |

Rules:

- **Normalized 0..1 space** (x right, y down) everywhere. Map to pixels with
  `x * rect.width`, `y * rect.height`. Never bake pixels into the spec.
- **Thumb-on-viewer-left base.** Mirror x for the opposite presentation;
  never mirror text labels, only geometry.
- **Fill once, stroke per finger.** `closed_loop_order` chains end-to-start
  back to the first move (the gate checks this): fill the whole loop a single
  time for the glassy fill, then stroke `fingers.*` / `thumb` / `base.*`
  segments individually for draw-in and pulse animation. Per-finger fills
  would double-blend translucent alphas into visible seams.
- **Dash lengths are iOS points.** Android px values are exactly 2×
  (`android_px_rule`); that is density scaling, not drift — do not "fix" it.
- **Theme hexes are law.** `overlay_theme.json` records resolved drifts
  (`_resolved`); aligning code to the JSON is correct, editing the JSON to
  match drifting code is not.
- Layout metrics (insets, corner radii, type sizes, icons) stay
  platform-native on purpose; only identity, geometry, color, alpha, timing,
  and gate values are shared.
- Run `./scripts/check_shared_shapes.py` to validate (CI fails on error).
  It also cross-checks the spec against the Swift/Kotlin sources so the JSON
  and the code it replaces cannot silently diverge during migration.
