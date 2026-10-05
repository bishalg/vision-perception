# models/asd_age — Subject Age Band Model Artifacts

Artifacts for the assistive subject-age check. **No binaries are committed** —
they are produced locally by `tools/asd_age_model/` because the training data
(UTKFace) is research-use licensed.

| File | Producer | Consumer |
| --- | --- | --- |
| `age_band_mobilenetv3.pt` | `train.py` | `convert_coreml.py`, `validate.py` |
| `AgeBand.mlpackage` | `convert_coreml.py` | `xcrun coremlcompiler compile` |
| `AgeBand.mlmodelc` | `coremlcompiler` (compiled) | `AgeBandEstimationEngine.discoverCompiledModelURL()` |

Install locations for `AgeBand.mlmodelc`, in discovery order:

1. App bundle resource (`AgeBand.mlmodelc`) — for TestFlight/App Store builds.
2. `Application Support/CurvAsdGazeKit/Models/AgeBand.mlmodelc` — sideload path
   for internal iterations, no app update required.

## Interface contract (`AgeBandEstimationEngine`)

- Input: `image` — 224×224 upright face crop (BGRA pixel buffer, scale 1/255,
  ImageNet normalization baked into the graph).
- Output: `probabilities` (dictionary keyed by `toddler`/`preschool`/`child`/
  `teenAdult`) + `classLabel`.
- Engine falls back through `ageBandProbabilities` → `probabilities` → `classLabel`.

## Licenses

- Backbone: MobileNetV3-Small via torchvision (BSD-3-Clause).
- Training data: UTKFace — research use only; the compiled model shipped in the
  app must be trained exclusively on data with cleared licensing (UTKFace today,
  app-consented collection data later). Record the provenance of the shipped
  checkpoint here before any external release.
