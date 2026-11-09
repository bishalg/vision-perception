# Handoff: EarlyGaze App Store readiness (as of 2026-11-09)

Read first: `store-collection-build.md` (the store build, decisions S-1..S-27), `i18n-readiness.md`,
`public-site-alignment.md`, `work-packages.md` (file ownership and traps for the older packages),
`technical.md`, `acceptance.md`, `qa-validation.md`, ADR-0001..0005. Tracker:
`docs/roadmap_2026q4/04_asd_phased_work_packages.md` (rows WP-A.*, WP-S.*, WP-LN.*, WP-W.1).
Working rules: spec before code; test-first; implementers do not commit; the main session runs every gate
and commits what passed; builds only under `/private/tmp/...`. Model routing per the repo `CLAUDE.md`.

The 2026-10-24 handoff (phase 1 blockers, phases 2-4 of the original plan) is history: those packages
landed as WP-A.1..A.6 and are recorded in the tracker. This file now holds only what matters for the
first submission.

## State

Branch `dev`. The App Store build (`Release`, `ASD_STORE_BUILD`) is an observation-capture tool: no
score, band, flag or report on the device (S-1..S-9). Landed: WP-S.1..S.5, S.7, LN.1, LN.3; WP-W.1 is on
the landing repo's `dev` branch awaiting the owner's merge to `main`. The listing is en-US only (S-14);
hi and ne-NP texts are parked in `fastlane/metadata_pending/`.

Identity: bundle id `com.open-spectrum.eye-gaze`, widget `com.open-spectrum.eye-gaze.carewidget`,
App Store Connect app id 6828468388 (record currently named "Eye Gaze"; the rename to "EarlyGaze" is a
metadata change on this version, S-7). Display name "EarlyGaze". `MARKETING_VERSION` 1.0.0, build 3,
iOS 18.0+. Category HEALTH_AND_FITNESS. Privacy manifest: no tracking, no collected data (true while the
default build sends nothing off the device, S-4). Entitlements: the store build (Release, StoreSim) signs with
`Resources/AsdGazeCollector-Store.entitlements` (Declared Age Range only, S-22); Debug and DemoRelease keep
HealthKit and Declared Age Range.

Gates (all in `scripts/ios_test.sh`; run from the repo root): kit tests, kit TSAN, XcodeGen, app tests
on a simulator, Release and DemoRelease builds, `check_asd_claims.py --swift --prose --secrets`, privacy
API check, `asd-cli i18n lint` against `scripts/i18n/literal-baseline.txt`, `asd-cli i18n check` on
both catalogues, `check_store_entitlements.sh` (S-22), `check_release_symbols.sh` for Release and DemoRelease. Plus
`fastlane ios release_check` (S-27: `validate_metadata`, `validate_privacy_manifest`,
`check_store_entitlements.sh`, `ruby -c Fastfile`). Last full green run: 2026-11-09 (WP-S.5b integration). Note: before 2026-11-09 the `ios_test.sh` claims step ran only the secrets scan (flag exclusivity in the script); the claims scan itself was green when run on its own.

## Before submission: code

| Item | Status | Where |
| --- | --- | --- |
| WP-S.5b opt-in gating (every toggle gates its feature; store build shows only the opt-ins whose feature exists; clinic delivery authorised by guardian consent + opt-in + enrolment) | done 2026-11-09 | `store-collection-build.md` S-15..S-21 |
| App Review notes sent with the metadata (`fastlane/review_notes.txt` -> `app_review_information[:notes]`), checked by the claims gate | done 2026-11-09 | `fastlane/Fastfile` `update_metadata` |
| WP-S.7b polish: plain-language Settings rows (CoreMotion filter, vault wording), iPad readable-width consent layout, neutral sync status text | done 2026-11-09 | `store-collection-build.md` S-23, S-25, S-26 |
| Store entitlements without HealthKit (Release and StoreSim sign with `AsdGazeCollector-Store.entitlements`; no HealthKit object at launch), static gate in `ios_test.sh` | done 2026-11-09 | S-22, `scripts/check_store_entitlements.sh` |
| Store-build delete-all: one confirmation removes sessions, queue, consent record, opt-ins and child profile | done 2026-11-09 | S-24, `DataDeletionService.deleteEverything()` |
| `fastlane ios release_check` (local pre-submission check) | done 2026-11-09 | S-27, first line of the runbook |
| Regenerate screenshots before upload (PNGs are git-ignored; the S.5b run on 2026-11-09 produced all 16) | at submission time | `apps/ios_asd_gaze_app/scripts/run_store_screenshots.sh` |
| README drift sweep (`apps/ios_asd_gaze_app/README.md`) | done 2026-11-09 | |
| WP-LN.2 literal burn-down of store-build files, WP-LN.4 pseudo-locale run | nice to have for en-US; needed before hi/ne listing | `i18n-readiness.md` |
| WP-S.6 shared consent text in the catalogue + `asd-contract` | after submission unless hi/ne listing moves first | `i18n-readiness.md`, `android-parity-plan.md` |
| WP-S.5c facial-expression metrics droppable per session in the kit, then offered as an opt-in | after submission | S-21 |

## Before submission: human (owner)

1. **H-6 account.** Confirm the Apple team, App Store Connect team and API key in the environment
   (`APPLE_TEAM_ID`, `ITC_TEAM_ID`, `ASC_KEY_ID`/`ASC_ISSUER_ID`/`ASC_KEY_CONTENT`), `APP_IDENTIFIER`
   unset or `com.open-spectrum.eye-gaze`. Enable the **Declared Age Range** capability on the App ID
   (the entitlement is in the build; without the capability the archive fails to sign).
   Decide the publisher name shown on the listing and the privacy page (copyright reads "Curv
   Engineering"; the site is "Open Spectrum Tracking").
2. **HealthKit entitlement in the store build.** Resolved in code (S-22): Release signs with
   `Resources/AsdGazeCollector-Store.entitlements` (Declared Age Range only). The App ID and the
   "Eye Gaze AppStore" profile may keep the HealthKit capability (a build may sign with a subset).
   `fastlane/review_notes.txt` says the store build carries no HealthKit entitlement. Nothing left to do.
3. **Public pages.** `https://open-spectrum-tracking.vercel.app/earlygaze`, `/privacy/earlygaze` and
   `/support/earlygaze` are live from `main` (checked 2026-11-09). The landing repo `dev` is ahead
   (effective date, retention defaults, enrolment steps; commit b2f19fe): merge `dev` -> `main`. Three
   owner placeholders remain in `apps/web/landing/content/earlygaze.ts` and keep the pages `noindex`:
   publisher legal name and postal address (H-6), hosting region and provider (O-7), support response time. Legal review of the privacy policy text. Replace the support email
   `support@curv.engineering` in the descriptions if the domain changes.
4. **Review contact.** Set `REVIEW_CONTACT_FIRST_NAME`, `REVIEW_CONTACT_LAST_NAME`, `REVIEW_CONTACT_EMAIL`,
   `REVIEW_CONTACT_PHONE` before `fastlane update_metadata` (never committed, CI-4). No demo account is
   needed (the app has no login); `review_notes.txt` says so.
5. **H-8 stimulus clips.** The clips are git-ignored and absent from a clean checkout, so a clean archive
   ships without them. Confirm the licence, place the licensed clips, and archive from a tree that has them
   (or ship the on-demand resource path). The capture flow blocks on the download gate otherwise.
6. **U7 real-device checklist** (`qa-validation.md`, acceptance scenarios 1, 10-16, 19): file protection
   of vault, sync queue and logs; consent flow; camera primer; a full capture on an iPhone with Face ID;
   the Declared Age Range prompt on a device signed into a child account.
7. **App Store Connect form.** Age rating questionnaire; App Privacy answers = "Data Not Collected" for this
   build (matches the manifest); export compliance (`ITSAppUsesNonExemptEncryption` is set in the plist);
   content rights (clips, H-8); the listing rename "Eye Gaze" -> "EarlyGaze".
8. **Old secrets.** The 2026-10 handoff recorded a committed password still in history on `origin` and the
   `cursor` remote (`fastlane/SIGNING_AND_SECRETS.md`, "History cleanup", decision H1: no rewrite). Confirm it was
   rotated; nothing in the current tree uses it.
9. **Native hi/ne review** of the parked listing texts and of the `needs_review` catalogue strings: not
   needed for the en-US submission; needed before the hi/ne listing or in-app review-quality translations.

## Submission runbook (after the code items above land and the gates are green)

```bash
cd apps/ios_asd_gaze_app
fastlane ios release_check                      # S-27: metadata, privacy manifest, store entitlements, Fastfile syntax
fastlane ios validate_metadata          # lengths, URLs, claims mirror
bash scripts/run_store_screenshots.sh           # 16 en-US captures from the StoreSim build
fastlane ios appstore_precheck          # Apple precheck rules on the metadata
fastlane ios sync_store_listing         # metadata (+ review info from env) and screenshots
fastlane ios beta                       # archive Release, upload to TestFlight (build number bump)
# TestFlight install on a Face ID device, run U7, then:
fastlane ios submit_app_store           # submit the processed build, manual release
```

## Traps that still apply

- `xcodegen generate` rewrites the tracked `project.pbxproj` and drops resources whose files are absent
  (the git-ignored clips in a fresh tree). Do not commit a pbxproj regenerated in such a tree.
- `xcodebuild test` occasionally hangs at launch; `test-without-building` after a build worked.
- Kit tests write temp files; in a sandboxed agent session 11 of them fail for that reason only. Run
  `swift test --package-path domains/asd_screening/ios` unsandboxed to confirm.
- The claims script scans `Text(`/`Label(` call lines and the metadata; model strings and new prose files
  need adding to its lists (`check_asd_claims.py`) to be covered.
- `StoreSim` is a debug-type configuration for the simulator only; it is never archived.
  `check_release_symbols.sh` proves the fixture launch keys are absent from Release and DemoRelease.
- Delete-all: observation data is deleted when the first dialog is confirmed; answering "Keep care plans"
  on the second prompt does not undo it (non-store builds). The store build has one confirmation that also
  removes the consent record, opt-ins and child profile (S-24).
- Docker Desktop's disk is too small for the `rust/Dockerfile` build; that build runs in CI only
  (unrelated to iOS, but it bites anyone running the full repo gates locally).
