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
| `screenshots/` | Output of the runner (not committed) |

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
- The parent-consent switch is a standalone, explicitly labelled SwiftUI
  `Toggle` carrying `accessibilityIdentifier("onboarding_consent_switch")`
  (added in OnboardingFlowView for assistive tech and automation). The flows
  tap it by id — no screen coordinates anywhere in the suite.
- Live gaze capture cannot run on a simulator (no TrueDepth camera); flow 03
  asserts the honest gate screen instead of a fake observation.

## Adding a screen

1. Add a Maestro flow step (or a new flow file, keeping the naming prefix order).
2. Mirror the journey in a `.feature` file under `../Tests/bdd/features/`.
3. If the screen should appear on the project website, add the screenshot name
   to the curated list in `scripts/run_maestro_screens.sh`.
