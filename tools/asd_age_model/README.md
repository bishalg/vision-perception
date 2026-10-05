# ASD Subject Age Model Tooling

Offline tooling that produces the on-device CoreML model consumed by
`CurvAsdGazeKit.AgeBandEstimationEngine` (subject-age check in the ASD gaze app).

The model is a **coarse 4-band classifier** — never an exact age regressor:

| Band | Months | Rationale |
| --- | --- | --- |
| `toddler` | 12–36 | matches the app's 12–35 mo screening brackets |
| `preschool` | 37–72 | matches the 36–60 mo brackets (one-band tolerance) |
| `child` | 73–156 | older siblings occasionally appear on camera |
| `teenAdult` | 157+ | the "adult holding the device" separation |

Public age models overestimate young children by years; the app's bracket
boundaries (12/18/24/36/48 months) are finer than that error. A coarse band
classifier with head-and-shoulder context is the deployment-honest design.

## Pipeline

```bash
pip install -r tools/asd_age_model/requirements.txt

# 1. Train (UTKFace layout: <age>_<gender>_<race>_<ts>.jpg)
python tools/asd_age_model/train.py --data-dir data/utkface --epochs 12

# 2. Validate — blocks shipping if toddler/teenAdult separation fails
python tools/asd_age_model/validate.py --checkpoint models/asd_age/age_band_mobilenetv3.pt --data-dir data/utkface

# 3. Convert to CoreML (classifier output: probabilities + classLabel)
python tools/asd_age_model/convert_coreml.py --checkpoint models/asd_age/age_band_mobilenetv3.pt

# 4. Compile and install for engine discovery
xcrun coremlcompiler compile models/asd_age/AgeBand.mlpackage .
cp AgeBand.mlmodelc "/path/to/app-sandbox/Application Support/CurvAsdGazeKit/Models/"
```

`AgeBandEstimationEngine.discoverCompiledModelURL()` looks for `AgeBand.mlmodelc`
in the app bundle first, then in `Application Support/CurvAsdGazeKit/Models/`
(the sideload path — no app update needed for model iterations).

## Dataset notes

- **UTKFace** (`susanqq.github.io/utkface`): ages 0–116, includes young children;
  out-of-scope faces (infants < 1 yr, noisy > 90 yr labels) are dropped in
  `band_from_years`. UTKFace is distributed for research use — do not ship the
  dataset itself.
- **Class balance**: teenAdult dominates UTKFace; `balanced_sampler` upweights
  young-child bands. Confirm the per-band distribution in the training log.
- **Optional teacher**: [MiVOLO](https://github.com/WildChlamydia/MiVOLO)
  (Apache-2.0, YOLOv8-based detector + VOLO transformer) can label/audit crops
  offline. It is too heavy to ship on the ANE; use it to cross-check our model
  or to pseudo-label the app's own consented collection data later.

## Future fine-tune loop

The app's norm-collection mode gathers consented session videos with age-bracket
metadata. Those frames (extracted crops only, with guardian consent) are the
correct fix for the child-accuracy gap and should replace/augment UTKFace in a
later iteration — see `docs/asd_subject_age_check.md`.
