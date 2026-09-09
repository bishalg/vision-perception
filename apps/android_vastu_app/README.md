# Android Vastu Perception Native App (`apps/android_vastu_app`)

Floor-plan / Vastu capture app built on the shared **`:overlay`** Android library (`apps/mobile_overlay/android`).

## Shared vs domain-specific

| Layer | Owns |
| :--- | :--- |
| `:overlay` (`org.curv.overlay`) | CameraX session, Liquid Glass HUD, 5-gate guidance |
| `org.curv.overlay.vastu` | Mandala reticle, vastu copy, `floor_plan` domain id, quad ROI |
| This app | ViewModel, result sheet, CurvPipeline handoff |

## Building

```bash
./scripts/build_android_aar.sh
# Open apps/android_vastu_app in Android Studio (API 26+)
```
