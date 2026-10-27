# Notices

## Stimulus videos (`Resources/Videos`)

Ten short clips ship in the app bundle. Their catalog entries (titles, age ranges, areas of interest) are in
`domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources/stimulus_catalog.json`.

| File | Measured duration (s) | Catalog `durationSec` | Status / Licence |
| --- | --- | --- | --- |
| `stim_01_social_peekaboo_8s.mp4` | 8.0 | 8.0 | Internal Research / Rights Unverified (H-8) |
| `stim_02_gaze_cue_right_8s.mp4` | 8.0 | 8.0 | Internal Research / Rights Unverified (H-8) |
| `stim_03_gaze_cue_left_8s.mp4` | 8.0 | 8.0 | Internal Research / Rights Unverified (H-8) |
| `stim_04_surprise_giftbox_10s.mp4` | 8.0 | 8.0 | Internal Research / Rights Unverified (H-8) |
| `stim_05_geopref_split_10s.mp4` | 8.0 | 8.0 | Internal Research / Rights Unverified (H-8) |
| `stim_06_dynamic_gears_marbles_8s.mp4` | 8.0 | 8.0 | Internal Research / Rights Unverified (H-8) |
| `stim_07_cartoon_slapstick_penguin_8s.mp4` | 8.0 | 8.0 | Internal Research / Rights Unverified (H-8) |
| `stim_08_recenter_bubble_fox_3s.mp4` | 8.0 | 8.0 | Internal Research / Rights Unverified (H-8) |
| `stim_09_social_turn_taking_10s.mp4` | 8.0 | 8.0 | Internal Research / Rights Unverified (H-8) |
| `stim_10_animal_puppy_play_8s.mp4` | 8.0 | 8.0 | Internal Research / Rights Unverified (H-8) |

Provenance: **unverified**. What the repository does show:

- The files carry the container tag `encoder=Google`, which names the encoder only.
- `docs/research/stimulus_taxonomy_and_prompts/` holds text prompts written for AI video generation and states
  that the camera-locked prompts were prepared for a generation queue. No record links a prompt to a
  specific shipped file, a generation tool, a model version, or a generation date.
- No source URL, licence text, terms-of-service reference, or model-release record for any person or likeness
  appearing in a clip is stored with the files.

Licence: **unknown / not confirmed**. No licence is granted or implied by this notice. Before a public
release, the publisher must record for each clip: the generating tool and its terms in force on the
generation date, whether commercial use and distribution inside an app are permitted, and whether the clip
depicts real or synthetic people. Until then, treat the clips as internal-use material.

Note: every shipped file measures 8.0 s, and all catalog entries have been aligned to 8.0 s (WP-D.1). File names retain legacy tags for asset tracking continuity.

## Third-party software

CareKit 4.1.0 (BSD-3-Clause, carekit-apple/CareKit) is linked through Swift Package Manager. Its licence text
ships in the package. This list is not an audit of every transitive dependency.

## M-CHAT-R/F

The M-CHAT-R/F questionnaire is off by default. It is compiled in only when the project is generated with
`MCHAT_LICENSED=1`, and that must happen only after written permission from the instrument's owners is
on file (decision H3, not yet confirmed).
