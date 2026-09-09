# iOS Vastu Perception Native App (`apps/ios_vastu_app`)

Floor-plan / Vastu capture app built on the shared **CurvCameraOverlay** kit.

## Shared vs domain-specific

| Layer | Owns |
| :--- | :--- |
| `apps/mobile_overlay/ios` (`CurvCameraOverlay`) | Camera session, Liquid Glass HUD, 5-gate guidance |
| `CurvVastuOverlay` | Mandala reticle, vastu copy, `floor_plan` domain id, quad ROI |
| This app | ViewModel, result sheet, CurvEngine handoff |

## Building

```bash
./scripts/build_ios_xcframework.sh
# Open as Swift Package / Xcode project and link CurvEngine.xcframework
```
