# Multilanguage readiness for the EarlyGaze iOS app (2026-11-09)

Status: design record and work-package plan. WP-LN.1 and WP-LN.3 landed 2026-11-09 (see section 7); WP-LN.2, WP-LN.4, WP-S.6 open. Owner decision 2026-11-09: the first App Store submission is
English-only (store-collection-build.md decision S-14), and the app itself must be fully multilanguage-ready
so that Hindi and Nepali can be switched on by a metadata move and a native review, not by code changes.

Binding inputs: `store-collection-build.md` (S-1..S-14), `android-parity-plan.md` (ANDP-9 shared text),
`rust/crates/asd-contract/src/i18n.rs` (WP-L.0 domain catalogue), `scripts/i18n/literal-baseline.txt`,
`scripts/ios_test.sh` (i18n lint step), CLAUDE.md (shared text and rules live in Rust; UI stays native).

## 1. State on 2026-11-09 (read-only audit)

| Area | Finding |
| --- | --- |
| Mechanism | `Sources/UI/AppLocalizer.swift`: in-code dictionary, `AppLocale {en, ne, hi}`, 161 keys, every key has hi and ne. Language follows `Locale.preferredLanguages`; no in-app picker. Fallback: requested language, then English, then the key. |
| Apple catalogues | None. No `.xcstrings`, `.strings` or `.lproj`. `project.yml` declares no `knownRegions`, `CFBundleLocalizations` or `CFBundleDevelopmentRegion`. iOS therefore offers no per-app language setting and the App Store lists only English. |
| Consent layer | `ConsentL10n.text(key, english)` returns English for every language; 230 call sites, 219 unique keys, across the consent flow, primers, opt-ins, Privacy & Your Data, Before You Begin, onboarding and the result screen. |
| Permission prompts | Seven `INFOPLIST_KEY_NS*UsageDescription` strings in `project.yml`, English only. No `InfoPlist.xcstrings`. |
| Literal lint | `asd-cli i18n lint` with a baseline of 404 user-visible literals, all in `Sources/Views` (402) and `Sources/App` (2). Store-build surfaces in the baseline: `SettingsView` 33, `OnboardingFlowView` 24, `GazeSessionResultView` 20, plus smaller counts. The rest are in care, M-CHAT, mood, demo and research views that the store build does not compile. |
| Formatting | 82 `String(format:)` uses with no locale (C locale decimal separator); 22 `DateFormatter` uses, none with a hard-coded user locale (the two `en_US_POSIX` uses are machine formats and correct). |
| Widget | `Widget/CareCountdownLiveActivity.swift` shows titles set by `CareAlertScheduler`, which never calls the localizer. Not compiled into the store build's visible features. |
| Tests | Three tests touch the localizer (key parity for a few keys, missing-key behaviour, format survival). No "every key has hi and ne" test, no pseudo-locale run, no `ConsentL10n` test. |
| Rust | `asd-contract` has the WP-L.0 domain vocabulary catalogue (en 464, ne 464, hi 11) with `negotiate` and `asd-cli i18n {lint, check, export}`. The iOS app does not consume it. |

## 2. Decisions

| # | Decision |
| --- | --- |
| I-1 | **Two tiers of text.** Tier A, *shared text that must be identical on every platform and is part of what the guardian agrees to*: consent document, permission primers, opt-in descriptions, the mandatory disclaimer sentences, recording-quality labels, storage-state labels. Tier A lives in the Rust catalogue (`asd-contract`, WP-S.6) and is exported to each platform. Tier B, *platform chrome*: tab names, buttons, navigation titles, settings rows, errors. Tier B lives in the native catalogue of each platform. Nothing is translated at runtime through FFI; the app ships exported artefacts (same rule as WP-L.0). |
| I-2 | **Apple String Catalogs replace the in-code dictionary.** `Resources/Localizable.xcstrings` (Tier B plus the exported Tier A) and `Resources/InfoPlist.xcstrings` (usage descriptions, display name). `Localizer.tr(key)` stays as the only call-site API and becomes a thin wrapper over `String(localized:table:bundle:)`; `AppLocale` stays for tests and for the catalogue export. `project.yml` declares `knownRegions: [en, hi, ne]`, `CFBundleDevelopmentRegion: en`, `CFBundleLocalizations: [en, hi, ne]`. Language selection stays system-driven; once localisations are declared, iOS adds the per-app language row in Settings, and the app's Settings gains a "Language" row that opens it (no custom picker). |
| I-3 | **Translation state is explicit.** Every hi and ne value carries the xcstrings `state` (`needs_review` for machine drafts, `translated` only after native review, H-10). The App Store may list a language only when `asd-cli i18n check` reports 100% `translated` for that language and S-14 promotes its metadata out of `fastlane/metadata_pending/`. Until then the catalogue ships the drafts, so the app is usable in that language, with a one-line "translation under review" note on the About screen for that language. |
| I-4 | **Consent hash per language.** The consent record (ConsentRecordStore) gains `language` and the sha256 of the text actually shown. The English text is the authoritative version; a translation shows the sentence "If this translation and the English text differ, the English text applies." The consent `version` stays shared across languages; changing any language's text bumps the version (re-consent). Schema v1.9 `consent` object carries `{version, sha256, language}`. |
| I-5 | **Numbers and dates use the user's locale.** User-visible `String(format:)` is replaced by `formatted()` or `Text(value, format:)`; `String(format:)` is kept only for machine output (CSV, log lines, file names) with `locale: Locale(identifier: "en_US_POSIX")` written explicitly so the lint can tell the two apart. `DateFormatter` for user-visible dates uses `.formatted(date:time:)` or an explicit `Locale.current`. |
| I-6 | **Pseudo-locale is a gate.** The store screenshot runner gains `--pseudo`, which launches with `-NSDoubleLocalizedStrings YES` and `-AppleLanguages (en)`, runs the store flows and saves the shots under `maestro/screenshots/pseudo/`. The reviewer checks for clipping; any screen that clips is fixed with wrapping or `minimumScaleFactor`, never by shortening the English. No `.frame(width:)` on a `Text` that holds a translatable string. |
| I-7 | **Baseline only shrinks, and store-build files reach zero first.** `scripts/i18n/literal-baseline.txt` may never gain entries. A new check in `scripts/ios_test.sh` fails if any baselined literal is in a file compiled into the store build (list in section 4). The remaining care, M-CHAT, mood, demo and research views are burned down later or deleted with their features. |
| I-8 | **Android reads the same Tier A export.** `asd-cli i18n export --android` writes `values/strings.xml`, `values-hi/`, `values-ne/` for Tier A; Tier B for Android lives in its own `strings.xml` (ANDP-9). The export is deterministic and committed, so a CI diff shows text changes. |
| I-9 | **Hindi and Nepali only, for now.** No RTL language is planned; RTL support is not a gate. Adding a language later is a catalogue change plus S-14 promotion, with no code change. |

## 3. Work packages

| WP | Scope | Tier | Depends on |
| --- | --- | --- | --- |
| WP-LN.1 Catalogues | Create `Localizable.xcstrings` from the AppLocalizer dictionary (one-off script in `asd-cli i18n export --xcstrings`, then delete the dictionary); `Localizer.tr` wrapper; `InfoPlist.xcstrings` for the seven usage strings and display name with hi/ne drafts; `project.yml` regions and `CFBundleLocalizations`; Settings "Language" row; tests: every key present in all three languages, `Localizer.tr` fallback order unchanged, `BuildConfigurationTests` reads the catalogue not `project.yml`. Regenerate pbxproj with the mp4s moved aside. | impl (Sonnet) | none |
| WP-LN.2 Store-build burn-down | Route every baselined literal in store-build files (section 4) through `Localizer.tr`; add hi/ne drafts marked `needs_review`; remove their baseline entries; add the I-7 store-file check to `scripts/ios_test.sh`. | impl (Sonnet) | WP-LN.1 |
| WP-S.6 Shared text | Tier A text in `asd-contract` (`i18n/consent.{en,hi,ne}.json` plus primer, opt-in, disclaimer and quality labels), sha256 per language and version, `asd-cli i18n export --xcstrings --android`, iOS `ConsentL10n` reads the exported table, `ConsentRecordStore` gains `language` and the shown-text hash, session schema v1.9 `consent` object, console shows the language of consent. | senior (Opus) | WP-LN.1 |
| WP-LN.3 Formatting | Classify the 82 `String(format:)` and 22 `DateFormatter` uses; convert user-visible ones per I-5; machine ones get the explicit POSIX locale; lint rule in `asd-cli i18n lint` flags `String(format:` without a locale argument in UI call sites. | impl (Sonnet) | none |
| WP-LN.4 Pseudo-locale gate | `run_store_screenshots.sh --pseudo`; run on both devices; fix clipping; record the run in the design doc; add the pseudo run to the pre-submission checklist. | impl (Sonnet) | WP-LN.1, WP-S.7 |
| WP-LN.5 Widget | `CareAlertScheduler` titles and bodies through `Localizer.tr`; widget strings in the catalogue. Not in the store build's path; schedule after submission. | impl (Sonnet) | WP-LN.1 |
| H-10 Native review | Hindi and Nepali review of catalogue, consent and listing text by native speakers; set `state: translated`; then S-14 promotion of `fastlane/metadata_pending/{hi,ne-NP}` and re-capture with `--lang hi --lang ne-NP`. | human | WP-LN.1, WP-S.6 |

Order: WP-LN.1 and WP-LN.3 in parallel (no shared files besides `project.yml`, which only WP-LN.1 touches), then WP-LN.2 and WP-S.6 in parallel (WP-LN.2 edits views, WP-S.6 edits the consent models and Rust), then WP-LN.4. Gates after each: `scripts/ios_test.sh` (includes the i18n lint), `python3 scripts/check_asd_claims.py --swift --prose --secrets`, `check_release_symbols.sh`, `fastlane validate_metadata`.

## 4. Store-build files that must reach zero baseline entries (I-7)

`Sources/App/AsdGazeApp.swift`, `Sources/Views/SettingsView.swift`, `Sources/Views/OnboardingFlowView.swift`,
`Sources/Views/GazeSessionResultView.swift`, `Sources/Views/StoreSessionsView.swift`, `Sources/Views/SplashScreenView.swift`,
`Sources/Views/GazeScreeningWizardView.swift`, `Sources/Views/GazeCalibrationScreen*.swift`, `Sources/Views/MainGazeScannerView.swift`,
`Sources/Views/GazeStimulusScreeningView.swift`, `Sources/Views/NotDiagnosticFooter.swift`, `Sources/Views/Consent/*.swift`,
`Sources/Views/PrivacyDashboard*.swift`, and any view reachable without `ASD_DEMO_TOOLS`, `ASD_RESEARCH_CLASSIFIER` or care features.
WP-LN.2 confirms the list by building `StoreSim` and taking the set of compiled view files.

## 5. Acceptance (definition of "multilanguage-ready")

1. `xcodebuild -showBuildSettings` and the built `Info.plist` list `en`, `hi`, `ne` in `CFBundleLocalizations`; iOS Settings shows the per-app language row.
2. `asd-cli i18n check --xcstrings apps/ios_asd_gaze_app/Resources/Localizable.xcstrings` reports every key present in all three languages and prints the `translated` percentage per language.
3. The store flows run under `--pseudo` with no clipped text on iPhone 18 Pro Max or iPad Pro 13-inch.
4. `scripts/i18n/literal-baseline.txt` holds no entry for a store-build file.
5. The consent record and session schema carry `language` and the shown-text hash; the English hash matches the catalogue export.
6. Switching the device to Hindi or Nepali shows translated consent, primers, Settings and permission prompts, each with the "translation under review" note until H-10 lands.

## 6. Non-goals

- No in-app language picker (iOS per-app language is the native pattern).
- No runtime translation through FFI.
- No RTL work (I-9).
- No translation of stimulus clip content (there is none) or of the console (separate module).

## 7. Status (2026-11-09)

| WP | State | Notes |
| --- | --- | --- |
| WP-LN.1 | done | `Resources/Localizable.xcstrings` (163 keys), `Resources/InfoPlist.xcstrings`; `Localizer.tr` resolves `<lang>.lproj`, then `en.lproj`, then the key; `project.yml` already had `developmentLanguage: en` and XcodeGen emits `knownRegions` Base/en/hi/ne; `Config/Info-Additions.plist` carries `CFBundleDevelopmentRegion` and `CFBundleLocalizations`; `asd-cli i18n check --app-xcstrings` added (the existing `--xcstrings` flag means drift against the domain catalogue) and wired into `scripts/ios_test.sh`. hi/ne values are machine drafts (`needs_review`, 0% translated) pending H-10. |
| WP-LN.3 | done | 82 `String(format:)` classified: 74 user-visible pass `locale: Locale.current`, 8 machine pass `FormatLocale.posix` (`Sources/Models/FormatLocale.swift`); two "d MMM" patterns replaced by `formatted(.dateTime.day().month(.abbreviated))`; lint rule `swift-format-locale` (single-line calls only; a multi-line `String(` newline `format:` call is not detected, follow-up). |
| WP-LN.2 | open | store-build literal burn-down; the baseline still holds 404 entries. |
| WP-LN.4 | open | pseudo-locale run; its dependencies WP-LN.1 and WP-S.7 are done, so it can start. |
| WP-LN.5 | open | after submission. |
| WP-S.6 | open | Tier A shared text in `asd-contract`; consent hash per language. |
| H-10 | open | native hi/ne review; then S-14 promotion of `fastlane/metadata_pending/`. |
