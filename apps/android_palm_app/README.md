# Android Palm Perception Native App (`apps/android_palm_app`)

Palm analysis app on the shared **`:overlay`** library plus palm plugin.

## Modules

- `:overlay` → `apps/mobile_overlay/android` (camera, HUD, guidance)
- `:app` → palm ViewModel, ML Kit tracker, crease canvas, reading sheet

## Building

```bash
./scripts/build_android_aar.sh
# Open apps/android_palm_app in Android Studio (API 26+)
```
