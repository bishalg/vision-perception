# BDD + Maestro screen-capture suite (`apps/ios_asd_gaze_app/maestro`)

Executable behaviour-driven flows for the Pediatric Gaze Collector. The Gherkin
feature files in `../Tests/bdd/features/` are the specification; the Maestro flows
here are their executable form and double as the screenshot pipeline for the
project website.

## Layout

| Artifact | Purpose |
| --- | --- |
| `flows/01-mode-selection.yaml` | First-launch mode selection → screenshot + start of onboarding |
| `flows/02-onboarding.yaml` | Consent, child profile, activity download, dashboard entry |
| `flows/03-dashboard-and-guardians.yaml` | Dashboard, M-CHAT-R/F modal, pediatrician report, gaze gate, settings |
| `flows/04-care-and-data.yaml` | App-mode switch, milestone detail, mood diary, guarded data deletion |
| `../Tests/bdd/features/*.feature` | Gherkin source of truth for the same journeys |
| `flows/store/*.yaml` | Store build flows and App Store screenshots (see below) |
| `screenshots/` | Output of the runners (not committed) |

## Running

```bash
# everything: build, install, all flows, screenshots into maestro/screenshots/
./scripts/run_maestro_screens.sh

# custom device + output, and copy the curated set for the website
DEVICE="iPhone 18 Pro" ./scripts/run_maestro_screens.sh --copy-to /path/to/open-spectrum-tracking/apps/web/landing/public/ios-app
```

## Conventions

- `clearState: true` only in flow 01; later flows continue the same session so
  they can be run in sequence (matches the script order).
- `takeScreenshot` names are stable: they are referenced by the feature files and
  by the website copy step.
- Guardian consent (WP-S.5) is a visual consent flow opened by the
  `onboarding_consent_review` button; flow 02 pages through it with `consent_next`,
  types a name into `consent_guardian_name` and taps `consent_agree`. The flows
  tap controls by id — no screen coordinates anywhere in the suite.
- Live gaze capture cannot run on a simulator (no TrueDepth camera); flow 03
  asserts the honest gate screen instead of a fake observation.

## Store build flows and App Store screenshots (WP-S.7)

`flows/store/` drives the App Store build's UI (Capture / Sessions / Settings, no result on the
device) and produces the App Store screenshots. The Debug flows above target screens the store
build does not have (mode picker, dashboard, M-CHAT).

| Flow | Screenshots | Gherkin |
| --- | --- | --- |
| `store/01-consent-and-onboarding.yaml` | `01_consent_overview`, `02_consent_sign` | `store_consent_and_onboarding.feature` |
| `store/02-capture-entry.yaml` | `03_before_you_begin`, `04_camera_primer` | `store_capture_entry.feature` |
| `store/03-session-and-sessions.yaml` | `05_recording_quality`, `06_sessions` | `store_sessions.feature` |
| `store/04-privacy-and-settings.yaml` | `07_privacy_your_data`, `08_settings_about` | `store_privacy_and_settings.feature` |

```bash
# both devices (iPhone 18 Pro Max, iPad Pro 13-inch (M5)), en-US (the listing is English-only, S-14)
./scripts/run_store_screenshots.sh
# the parked hi / ne-NP listings, when they are promoted
./scripts/run_store_screenshots.sh --lang hi --lang ne-NP
# one device and language, reusing the last build
./scripts/run_store_screenshots.sh --skip-build --device "iPhone 18 Pro Max" --lang en-US
```

- Build: the `StoreSim` configuration (`ASD_STORE_BUILD` + `ASD_STORE_SCREENSHOTS`), built for a
  concrete simulator (the Rust static library is arm64-only).
- Flow 01 starts clean (`clearKeychain`, `clearState: true`); 02-04 continue from it, so run them
  in order (the runner does). Without the fixture, flow 01 asserts the honest hardware gate.
- Flows 02-04 launch with `-ASDStoreScreenshotFixture YES` (and 03 with
  `-ASDStoreScreenshotScreen result`): the StoreSim-only fixture lets the simulator stand in for a
  TrueDepth device and shows example sessions in memory (marked simulated, no scores). It is not
  compiled into Release or DemoRelease (`scripts/check_release_symbols.sh`).
- Every tap is by accessibility id (`tab_capture`, `tab_sessions`, `tab_settings`,
  `onboarding_continue`, `store_session_done`, `store_session_row`, ...), so the flows run in
  every language. The runner sets the simulator language and region (`AppleLanguages`,
  `AppleLocale`); the app follows the system language (`AppLocale.current`).
- Hard rules for every captured frame: no stimulus clip (H-8), no score, band, flag or report
  text, no simulation badge, no real name (the guardian is "A. Parent").
- Output: `fastlane/screenshots/<lang>/<Device Name>-NN_name.png` for `fastlane
  upload_screenshots`. These PNGs are git-ignored on purpose: regenerate them with the runner
  before an upload instead of committing them; Maestro logs and raw captures per run in
  `screenshots/<device>/<lang>/<flow>/` (git-ignored).

## Adding a screen

1. Add a Maestro flow step (or a new flow file, keeping the naming prefix order).
2. Mirror the journey in a `.feature` file under `../Tests/bdd/features/`.
3. If the screen should appear on the project website, add the screenshot name
   to the curated list in `scripts/run_maestro_screens.sh`.
