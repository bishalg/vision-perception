# iOS Palm Scanner (`apps/ios_palm_app`)

## Why you still saw "random" lines

`CurvEngine.xcframework` is **not linked** yet (needs OpenCV-for-iOS). Without it the
app falls back to landmark-guess creases. That is expected until native links.

## Edit lines for reading

On the result screen:

| Action | How |
| :--- | :--- |
| Reshape | **Edit** → drag vertex handles on the selected line |
| Reorder | Drag list rows (edit mode) |
| Reassign Heart/Head/Life/Fate | Tag menu on a row |
| Lock for reading | **Use these lines for reading** → `predictionReadySplines` / `predictionPayloadJSON()` |

Adjusted geometry is what future VLM/reading should consume — not the raw landmark guesses.

## Native engine (later)

```bash
# Requires iOS OpenCV install
OpenCV_DIR=/path/to/ios-opencv ./scripts/build_ios_xcframework.sh
# Package.swift auto-picks build/CurvEngine.xcframework
```

See [`docs/palm_crease_baseline.md`](../../docs/palm_crease_baseline.md) and
[`docs/handover_palm_crease_ios.md`](../../docs/handover_palm_crease_ios.md).
