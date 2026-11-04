# ASD localisation and iOS constants - Work Packages (WP-L.0, WP-L.1, WP-X.0c, WP-M.1)

Designed 2026-11-04 against `dev` at `e75053f` (ADR-0013 principles 6 and 7; ADR-0012 registry). MUST ids
(I18N-*, DSP-*) in `technical.md`; scenarios in `acceptance.md`; status tracker
`docs/roadmap_2026q4/04_asd_phased_work_packages.md` (§0 rows WP-L.0, WP-L.1, WP-X.0c, WP-M.1). Tier legend: `S`
= `senior-implementer` (Opus); `I` = `implementer` (Sonnet); size `S`mall (≤ 1 day) / `I`ntermediate (2–4 days) /
`L`arge (5–7 days) for one agent. WP-L.1 is delivered as one seam package (L.1.0) and five wave packages
(L.1a–L.1e); the tracker row WP-L.1 is done when all six are.

Rules that apply to every package here:

- Each package owns the files it lists and touches nothing else. `rust/Cargo.lock`, `rust/.sqlx/`,
  `docs/spec/schemas/asd/*.json` (except where a step-0 package is told to commit a first version),
  `apps/curv_console/package-lock.json`, `rust/README.md` and the tracker are edited by the main session at
  integration, never by a wave package. **Never stage `rust/Cargo.toml` unless the package lists it.**
- Migration number is fixed: **0028** (`0028_locale.sql`, owner WP-L.0). **0027 stays reserved for WP-B.21.** If
  L.0 lands before B.21, B.21 takes **0029** (the tracker rule is updated at integration). No other package adds
  a migration.
- The catalogue files `rust/crates/asd-contract/i18n/domain.*.json` are edited only by WP-L.0 and WP-X.0c (step 0).
  A wave package that needs a missing domain key reports it; the main session adds it and regenerates
  `domain.ts`. The generated `apps/curv_console/src/messages/domain.ts` is never hand-edited.
- Console work is **mock-first**: every console package passes `CURV_API_MODE=mock` (unit tests, build, e2e)
  before any live check; the live e2e runs only after L.1e is merged.
- No new Python, no new npm or crate dependency (Noto Sans Devanagari is a font file with its OFL licence, not a
  package). Every `ne` text written by an agent is a machine draft marked `// ne: draft` (console) or `"draft":
  true` (catalogue) for WP-L.3's native-speaker review.
- A package reports: files changed, test output, what was skipped and why, any MUST id it could not satisfy as
  written, and any literal it had to exempt (with the reason).

## Step 0 (serial): shared seams

Order: **WP-L.0** first; then **WP-L.1.0** and **WP-X.0c** in parallel (file-disjoint: console vs Rust). The wave
starts when all three are merged and the main session has regenerated the schemas, rebuilt `public/asd-wasm` and
the xcframework, and committed.

### WP-L.0 Domain catalogue, `asd-cli i18n`, locale contract, migration 0028 - `S` - size I

Goal: I18N-1..8, I18N-10..12 (contract half), I18N-4 (acceptance 1–5, 18).
Owner files:
- `rust/crates/asd-contract/src/i18n.rs` (new: `Locale`, `CatalogueLocale`, catalogue load, `text`, `render`,
  `required_keys`, `problems`, `negotiate`, `CATALOGUE_VERSION`, the I18N-3 checks, the completeness test),
  `rust/crates/asd-contract/i18n/domain.en.json`, `domain.ne.json` (new; every required key; `ne` drafts; the
  `param.*` texts **moved** from `apps/curv_console/src/messages/research.ts` `en`/`ne` `params` blocks, not
  rewritten), `rust/crates/asd-contract/src/claims.rs` (new: `DENY_TERMS`), `src/problem.rs` (`ALL_SLUGS`),
  `src/clinical.rs` (`UserProfile.locale`, `effective_locale`, `UpdateProfileRequest.locale`), `src/api_v1.rs`
  (`Org.default_locale`, `OrgPatch.default_locale`), `src/schema.rs` (`$defs` for `Locale`; i18n-v1 and
  negotiation-cases export), `src/lib.rs`, `rust/crates/asd-contract/tests/i18n.rs` (new).
- `rust/crates/asd-core/src/flag.rs` (`FlagCase`, `flag_case`; `assess` output unchanged),
  `rust/crates/asd-wasm/src/api.rs` (`flagCase`), `rust/crates/asd-wasm/tests/*` (one test).
- `rust/crates/asd-cli/src/cmd/i18n.rs` (new: `export`, `check`, `lint`, `fmt`), `src/cmd/mod.rs`,
  `src/main.rs`, `src/cmd/schema.rs` (`i18n-v1.json`, `i18n-negotiation-cases.json`),
  `rust/crates/asd-cli/tests/i18n_*.rs` (new; goldens under `tests/fixtures/i18n/`).
- `rust/crates/asd-backend/migrations/0028_locale.sql` (I18N-10, whole file), a migration test in
  `rust/crates/asd-backend/tests/migrations.rs` (or the existing migration test file) - **no route changes**
  (L.1e); the backend must still compile with the new optional contract fields (fill `locale: None`,
  `effective_locale: Locale::En` where `UserProfile` is built, nothing more).
- Committed first versions: `docs/spec/schemas/asd/i18n-v1.json`, `docs/spec/schemas/asd/i18n-negotiation-cases.json`,
  regenerated `api-v1.json`; `apps/curv_console/src/messages/domain.ts` (generated);
  `scripts/i18n/literal-baseline.txt` (generated first baseline).
- CI wiring: `.github/workflows/rust.yml` (two steps: `i18n check`, `i18n lint` over the iOS and Android roots),
  `scripts/ios_test.sh` (one static-gate step running the same lint).
Not to touch: console sources other than the generated `domain.ts` (L.1.0 removes the `params` blocks and the
generator script), backend routes (L.1e), iOS sources (M.1, L.2), `params.rs`/`config.rs` (X.0c).
Tests first: required-keys completeness both ways (delete one `ne` key → named failure; add an enum variant in a
test-only enum registered under a test group → named failure); placeholder mismatch, missing Devanagari, claims
term, unsorted file each produce one named problem; `FlagCase` constants equal the catalogue `en` values; every
existing `asd-core` golden byte-identical; `negotiate` passes every row of the fixture (≥ 20 rows incl. `q=0`,
`ne-NP;q=0.8,en;q=0.9`, `*`, empty, garbage); console/xcstrings/android exports equal their goldens; `check`
fails on a hand-edited `domain.ts`; lint: a fixture tree with `Text("Hello")` → 1 finding, `Text(verbatim:)`,
`String(localized:)`, `AppLocalizer.tr`, `// i18n-ok:` → none, a catalogued key with `--catalog` → none; baseline
covers → exit 0, new literal → exit 1, stale entry → exit 1, `--update-baseline` round-trip; migration 0028 up on
a 0026 database (no 0027) and on fresh, CHECK refuses `fr`, `settings.locale` carried and removed.
Commands: `scripts/rust_test.sh` (UTF-8 Postgres, `ASD_REQUIRE_ALL=1`), `cargo sqlx prepare --workspace` (step 0
runs it), `scripts/build_asd_wasm.sh --test`, `cargo run -p asd-cli -- schema export && cargo run -p asd-cli --
schema check`, `cargo run -p asd-cli -- i18n check`, `cargo run -p asd-cli -- i18n lint --swift
apps/ios_asd_gaze_app/Sources --swift domains/asd_screening/ios/Sources --kotlin apps/android_asd_gaze_spike
--baseline scripts/i18n/literal-baseline.txt`, `python3 scripts/check_asd_claims.py --swift --prose` (unchanged,
must stay green).
Depends on: X.0 (done). Everything below depends on it.

### WP-L.1.0 Console i18n seams - `I` - size S

Goal: I18N-14..17 and I18N-19 plumbing, I18N-9 at `warn`, I18N-21; no translation, no call-site migration
(acceptance 6, 7 in part).
Owner files (all under `apps/curv_console/`):
- `src/lib/i18n/{types.ts,messages.ts,server.ts,provider.tsx,negotiate.ts}` (new), `src/lib/i18n/*.test.ts`
  (negotiation fixture test reads `../../docs/spec/schemas/asd/i18n-negotiation-cases.json`).
- Every `src/messages/{en,admin,catalog,clinical,config,exports,participant,privacy,security}.ts`: **shape only**
  - wrap the existing tree as `const en = {…}` and add `const ne: Widen<typeof en> = { …copy of en… } // ne:
  TODO`, export `<file>Messages = { en, ne }`; keep the old named exports (`en`, `adminText`, …) as deprecated
  aliases of the `en` tree so no call site breaks. `src/messages/research.ts`: delete the `params` blocks and the
  `<registry:…>` markers; `ResearchMessages.params` is replaced by `dt(locale, spec.label_key)` at the one
  consumer (`src/components/research/RegistryForm*.tsx`: the only component edit in this package).
- `scripts/gen-registry-messages.mjs` (deleted) and its `package.json` script entry.
- `src/app/layout.tsx` (`resolveLocale`, `<html lang>`, `LocaleProvider`, `generateMetadata`, font class),
  `src/app/fonts/NotoSansDevanagari-*.woff2` + `OFL.txt` (new), `src/app/globals.css` (`:lang(ne)` stack).
- `src/lib/format.ts` (locale parameter with default `"en"`, unit words passed in; existing call sites compile
  unchanged), `src/lib/api/problem.ts` (new `problemText`), `src/lib/api/types.ts` (zod: `locale`,
  `effective_locale`, `default_locale` optional), `src/lib/api/clinical.ts` / `src/lib/api/admin.ts` (patch
  bodies), `src/lib/api/mock/*` (mock `/users/me` returns `effective_locale` from the mock user's `locale`, else
  the mock org's `default_locale`, else the `Accept-Language` match; PATCH stores it), `src/lib/api/*.contract.test.ts`.
- `eslint.config.mjs` (I18N-9 rules at `warn`, the `LITERAL_GATE_ERROR_DIRS` override list, initially
  `["src/app/fonts"]`-style empty-safe), `eslint-rules/no-literal-ui-props.mjs` + `.test.mjs` (new).
- `tests/messages/<file>.test.ts` (new, one per message file, each with `const NE_PENDING = true` that skips the
  `ne` assertions; the owning wave package sets it to `false`); the old `tests/messages.test.ts` research
  assertions move into `tests/messages/research.test.ts`.
- `e2e/locale.spec.ts` (new: `<html lang>` follows the mock user's locale; the crawl test of acceptance 9 is
  present as `test.fixme` and enabled at integration).
- `README.md` "Licences" (font line) and a short "Localisation" section.
Not to touch: any `src/app/**` page or `src/components/**` file other than those named; the Rust tree.
Tests first: `Widen` type test (a missing `ne` key is a `tsc` error: `// @ts-expect-error` fixture); negotiation
fixture; `resolveLocale` with and without a session (mock); `problemText` known/unknown slug; `format.ts` `ne`
uses Latin digits; custom ESLint rule `RuleTester` cases (`aria-label="Close"` → error, `aria-label={t.x}` → ok,
`` title={`Hi`} `` → error, `className="a b"` → ok).
Commands (in `apps/curv_console`): `npm test`, `npm run typecheck`, `npm run lint` (warnings allowed),
`CURV_API_MODE=mock npm run build`, `npm run e2e -- e2e/locale.spec.ts`, `npm run e2e -- --project=prod` (CSP clean
with the self-hosted font).
Depends on: L.0. Parallel with: X.0c.

### WP-X.0c Contract addition: display bands, session quality, age-band representative months, tolerant device acceptance - `S` - size I

Goal: DSP-1..7 (acceptance 12, 13, 17).
Owner files:
- `rust/crates/asd-contract/src/config.rs` (`DisplayParams`, `SessionQualityParams`, profile 1.3.0,
  `SCORING_PROFILE_SCHEMA_VERSION`), `src/params.rs` (16 rows, `AgeBands` accepts `representativeMonths`,
  `validate_for_device`), `src/profile_checks.rs` (DSP-1 cross-field rules), `src/mirror.rs` (schema mirrors),
  `rust/crates/asd-contract/i18n/domain.{en,ne}.json` (the DSP-7 keys only), `tests/params.rs`.
- `rust/crates/asd-core/src/display.rs` (new, DSP-3), `src/quality.rs` (`session_verdict`, DSP-4),
  `src/types/baselines.rs` (`AgeBracket.representative_months`), `src/norms.rs` (default brackets carry
  `None`; the profile default sets the values in `asd-contract`), `src/lib.rs`, `tests/display.rs` (new).
- `rust/crates/asd-sync/src/*` (the `accept_bundle` call switches to `validate_for_device`; one test with a 1.3.0
  profile carrying an extra unknown section `futureSection` → accepted).
- `rust/crates/asd-ffi/src/display.rs` (new, DSP-5), `src/lib.rs` (module), ffi tests.
- `rust/crates/asd-wasm/src/api.rs` (`displayBand`, `sessionQualityVerdict`), one wasm test.
- Committed: regenerated `param-registry-v1.json`, `config-bundle-v1.json`, `api-v1.json`, `i18n-v1.json`, and the
  regenerated `apps/curv_console/src/messages/domain.ts` (generated file; the only console file touched).
Not to touch: iOS sources (M.1), console sources other than `domain.ts`, backend routes. The backend must compile
(it reads `ScoringProfile`; no behaviour change: it never reads `display`/`sessionQuality`).
Tests first: PRM-2 completeness with the 16 new rows; 1.2.0 body decodes to the DSP-1 defaults; `params::validate`
refuses `display.bands.eyeDwellPct.okMin = 120`, `monitorMin > okMin`, `representativeMonths` outside its band;
`validate_for_device` accepts an unknown top-level key and still refuses an out-of-range known one, while
`validate` (publish) refuses the unknown key; DSP-3 boundary table (every value in technical §5.1 #4–#19 at, just
below and just above each cut-off, matching today's Swift operators); DSP-4 table (0.75/0.749/0.50/0.499 ×
interrupted); FFI round-trip `display_params_from_profile(default) == defaults`; `age_bands_from_profile(default)`
gives representative months 14, 20, 28, 40, 54; every `asd-core` golden, kit `GoldenParityTests` input and
Android `GoldenParityTest` byte-identical; `i18n check` green with the new keys.
Commands: `scripts/rust_test.sh`, `scripts/build_asd_wasm.sh --test`, `scripts/build_asd_android.sh --test`,
`cargo run -p asd-cli -- schema export && … schema check`, `cargo run -p asd-cli -- i18n check`,
`cargo run -p asd-cli -- params list --stage display` (lists the 14 `display.*` rows plus the existing
`Display`-stage rows).
Depends on: L.0 (catalogue keys). Parallel with: L.1.0.

## Wave 1 (parallel, file-disjoint)

Directory ownership for the console packages (every `src/app` and `src/components` path belongs to exactly one
package; a `src/lib/*` file not owned by step 0 belongs to L.1a; a package may **read** another package's message
file through `getText()`/`useText()` but edits only its own):

| Package | Message files (writes the `ne` tree) | `src/app/**` | `src/components/**` | tests / e2e |
| --- | --- | --- | --- | --- |
| L.1a | `en.ts` | `page.tsx`, `not-found.tsx`, `login/`, `switch-org/`, `[org]/layout.tsx`, `[org]/error.tsx`, `[org]/not-found.tsx`, `[org]/overview/`, `[org]/sessions/`, `[org]/review/`, `[org]/devices/`, `[org]/audit/` | `AuditFilters`, `AuditTable`, `ClipsTable`, `DevicesTable`, `Distribution`, `DriftChart`, `EnrolCodePanel`, `FlagBadge`, `KpiCard`, `LossBar`, `NavLinks`, `OrgSwitcher`, `Pagination`, `QualityPanel`, `ReportPanel`, `ReviewQueue`, `ScoringStatus`, `SessionFilters`, `SessionsTable`, `SessionTabs`, `TrackingQuality` | `tests/messages/en.test.ts`, the tests of those components, `e2e/{login,session,review,devices,audit}.spec.ts` |
| L.1b | `participant.ts`, `clinical.ts` | `[org]/participants/`, `[org]/worklist/`, `[org]/follow-ups/` | `participant/`, `clinical/`, `ParticipantsTable`, `QrCode` | `tests/messages/{participant,clinical}.test.ts`, their component tests, `e2e/{participant,clinical}.spec.ts` |
| L.1c | `exports.ts`, `privacy.ts`, `security.ts` | `[org]/exports/`, `[org]/erasure/`, `[org]/settings/` (incl. the I18N-20 selectors), `[org]/privacy-actions.ts`, `share/`, `api/` | `exports/`, `privacy/`, `security/` | `tests/messages/{exports,privacy,security}.test.ts`, their component tests, `e2e/{exports,privacy,security,share}.spec.ts` |
| L.1d | `admin.ts`, `catalog.ts`, `config.ts` (research `ne` exists) | `[org]/users/`, `[org]/config/`, `[org]/research/` | `catalog/`, `config/`, `research/`, `UsersAdmin` | `tests/messages/{admin,catalog,config,research}.test.ts`, their component tests, `e2e/{catalog,config,research}.spec.ts` |

Common to L.1a–L.1d (each for its own files):
- Replace static message imports with `const t = await getText()` (server) or `const t = useText()` (client)
  (I18N-16); pass the locale to `format.ts`; render enums through `enumLabel`/`dt` and delete the package's
  enum-label maps (I18N-18); render problems through `problemText` (I18N-19).
- Write the `ne` tree of the owned message files (draft, `// ne: draft`), set `NE_PENDING = false` in the owned
  `tests/messages/*.test.ts`, fill `NE_SAME_AS_EN` with justified entries only.
- Clear every `react/jsx-no-literals` and `curv/no-literal-ui-props` warning in the owned directories, then append
  them to `LITERAL_GATE_ERROR_DIRS` in `eslint.config.mjs`. **That one array is the only shared line**: each
  package appends its own entries in a separate block comment `// L.1x` so the main session merges the four
  blocks mechanically.
- Add one Nepali assertion to each owned e2e spec (set the mock user's locale to `ne`, assert a page heading in
  Devanagari and `lang="ne-NP"`).
- Commands (in `apps/curv_console`): `npm test`, `npm run typecheck`, `npx eslint <owned dirs> --max-warnings 0`,
  `CURV_API_MODE=mock npm run build`, `npm run e2e -- <owned specs>`.

### WP-L.1a Console shell, sessions, review, devices, audit in `ne` - `I` - size I

Goal: I18N-15..19 for the files above; flag headline/detail via `flagCase` (I18N-4/18) in `ReportPanel`,
`FlagBadge`, the session pages; `<html lang>` already done in L.1.0; nav labels; `formatAge` units.
Not to touch: other packages' rows; `src/lib/i18n/**`, `format.ts`, `problem.ts` (step 0; report a needed change).
Tests first: `tests/messages/en.test.ts` with `NE_PENDING = false` fails until the `ne` tree is complete; a
component test renders `SessionsTable` with `ne` and finds "Yes/No" replaced (ADR-0013 consequence: the
`SessionsTable` literal leak); `FlagBadge`/`ReportPanel` render `flag_assessment.matches.headline` in `ne` for a
session whose headline is the English "Looking matches the age reference", and fall back to the sent text for an
unknown headline.
Depends on: step 0. Parallel with: L.1b–e, M.1.

### WP-L.1b Console participants, encounters, attachments, follow-ups, worklist, clinical review in `ne` - `I` - size I

Goal: as above for its rows; `encounterKinds`/`attachmentKinds`/`cosignPolicies`/`patientRefModes` maps deleted in
favour of `enumLabel`; participant `language` via `Intl.DisplayNames`; `age_bracket_at_enrol` via `age_band.range`
(band months from the effective profile, which the participant page already loads, else the raw id).
Tests first: participant page in `ne` shows `encounter_kind.screening_visit` text and no raw `screening_visit`;
clinical sign dialog errors render `problem.<slug>`; the ADR-0004 mandatory sentence appears in `ne` and English
on the clinical review page (I18N-23).
Depends on: step 0. Parallel with: L.1a, c–e, M.1.

### WP-L.1c Console exports, privacy, security, settings, share page in `ne`; language selectors - `I` - size I

Goal: as above for its rows, plus I18N-20 (user "Language" on `/[org]/settings/security`, org "Default language"
on `/[org]/settings/clinical`, server actions with `revalidatePath("/", "layout")`; strings in `security.ts`)
and I18N-22 (share page and its error keys via `Accept-Language`, English mandatory sentences kept).
Tests first: selector component test (three options, current value from `/users/me`), server action calls
`updateMyProfile({locale})` and `null` for "organisation default"; org selector hidden for `clinician`; share page
with `Accept-Language: ne` renders Nepali text plus the English "This is not a diagnostic tool.".
Depends on: step 0. Parallel with: L.1a, b, d, e, M.1.

### WP-L.1d Console users, catalog, config, research in `ne` - `I` - size S

Goal: as above for its rows; research pages switch from `researchText` (= `en`) to the locale's tree; role,
user status, config document kind/status, experiment/run enums via `enumLabel` (`UsersAdmin` status text leak).
Tests first: research experiment page in `ne` shows the Nepali registry label for `metrics.ivtVelocityUnitsPerSec`
from `domain.ts` (toolkit acceptance 3); `UsersAdmin` status column in `ne`.
Depends on: step 0. Parallel with: L.1a–c, e, M.1.

### WP-L.1e Backend: locale on users and organisations - `I` - size S

Goal: I18N-12 (server half), I18N-13 (acceptance 8).
Owner files: `rust/crates/asd-backend/src/routes/users.rs` (or the module serving `/users/me`),
`src/routes/orgs.rs` (or the module serving `/orgs/{id}`), the user/org row structs and queries they use,
`rust/crates/asd-backend/tests/locale.rs` (new).
Not to touch: the migration (step 0), `asd-contract` (step 0; report a gap), the console.
Tests first (`sqlx::test`, `tests/common`): `GET /users/me` → `locale: null`, `effective_locale: "en"` on a fresh
org; `PATCH /orgs/{id} {default_locale: "ne"}` as admin → 200, audit diff has `default_locale`; as clinician →
403; then `/users/me` → `effective_locale: "ne"`; `PATCH /users/me {locale: "en"}` → `effective_locale: "en"`,
audit `user.profile_update`; `{locale: null}` → back to `ne`; `{locale: "fr"}` → 400 `validation-failed`;
`superadmin` without home org with `Accept-Language: ne-NP` → `ne`; `PATCH /orgs/{id} {settings: {locale: "ne"}}` →
400 pointing at `default_locale`.
Commands: `scripts/rust_test.sh` (no `cargo sqlx prepare`: the main session runs it at integration).
Depends on: step 0. Parallel with: everything in the wave.

### WP-M.1 iOS display bands, age bands and constants from the bundle; grep gate - `I` - size I

Goal: DSP-8..11 (acceptance 14–17); toolkit acceptance 17 and 18.
Owner files:
- Kit: `domains/asd_screening/ios/Sources/CurvAsdGazeKit/ScoringConfiguration.swift` (`display`, `ageBands`),
  `DisplayBands.swift` (new thin wrapper over the FFI), `GazeProcessingActor.swift` (`maxFrameDwellSec` → profile
  `aoi.dwellCapMs`; verdict → `sessionQualityVerdict`), kit tests `Tests/CurvAsdGazeKitTests/DisplayBandsTests.swift`
  (new), `SessionVerdictTests.swift` (new).
- App: `Sources/Views/{PerVideoResultViews,EngagementRiskStratificationView,EngagementDetailViews,
  EngagementAffectAndQualityViews,GazeSessionResultView,CaregiverDashboardView,ResearchStudyConsentView,
  SettingsView,OnboardingFlowView,PediatricReportView,SubjectAgeQuickCheckCard,MainGazeScannerView,
  GazeStimulusScreeningView,GazeScreeningWizardView}.swift`, `Sources/Models/{ChildProfile,DoctorReportGenerator,
  OnDemandStimulusManager,ClinicalDashboardViewModel,SubjectAgeCheckController}.swift`,
  `Sources/UI/AppLocalizer.swift` (new keys only: `display.target.*`, `display_band.*`, `age_band.range`,
  `age_band.<id>.stage|focus`, `age_band.repick`; en/ne/hi), app tests `Tests/…/ChildProfileMigrationTests.swift`
  (new), `DisplayBandViewTests.swift` (new).
- Gate: `scripts/check_ios_constants.sh` (new), `scripts/ios-constants-allowlist.txt` (new),
  `scripts/tests/ios_constants/{bad,clean}/*.swift` (fixtures), `scripts/ios_test.sh` (one step).
Not to touch: Rust (X.0c owns the FFI; report a gap), `project.yml` localisation settings and any String Catalog
(WP-L.2), the 34 inventory sites' surrounding copy beyond the target strings (L.2 migrates literals), Debug-only
`Inference/*` (exempt, owner question 18).
Tests first: `DisplayBandsTests` - for each metric the default profile gives the same status as the old Swift
expression at the §5.1 boundary values (table-driven, the old expressions copied into the test as the oracle);
a profile JSON with `display.bands.eyeDwellPct.okMin = 30` turns 27 % from `ok` to `monitor`; a 1.2.0 profile
gives the defaults; `SessionVerdictTests` - 0.75/0.749/0.50/0.499 × interrupted equals the pre-change verdicts;
`ChildProfileMigrationTests` - a stored profile with `"ageBracket": "24–35 mo"` decodes to `24-35` with
representative months 28, re-encodes as the id, and `ageMonths` for a session is 28; an unknown id keeps its last
months and sets the re-pick flag; gate self-test: `bad/` fixture with `if eyePct >= 25.0` fails, `clean/` passes.
Commands: `bash scripts/check_ios_constants.sh --self-test`, `bash scripts/check_ios_constants.sh`,
`scripts/ios_test.sh` (rebuilds the xcframework with X.0c's FFI; kit, TSAN, Swift 6 warnings-as-errors, app,
claims, i18n lint with no new finding beyond the baseline - new view strings go through `AppLocalizer`).
Depends on: X.0c (FFI) and L.0 (lint step). Parallel with: L.1a–e. Must finish before WP-L.2 starts (both are
iOS-wide).

## Integration (main session, after the wave)

1. Merge the four `LITERAL_GATE_ERROR_DIRS` blocks, then replace them with `"error"` for both rules everywhere;
   `npm run lint -- --max-warnings 0`; add `no-restricted-imports` for `@/messages/*` (except `domain`) under
   `src/app/**` and `src/components/**`; delete the deprecated `en`/`adminText`/… aliases; enable the crawl test in
   `e2e/locale.spec.ts`.
2. `cargo sqlx prepare --workspace`, `schema export` + `check`, `i18n check`, rebuild `public/asd-wasm` and the
   xcframework.
3. Every gate of the tracker §10 list plus `bash scripts/check_ios_constants.sh`, `npm run e2e -- --project=prod`,
   live e2e for the settings selectors against `rust/docker-compose.yml`.
4. `rust/README.md`: a "Localisation seams" section (catalogue, CLI, negotiation, migration 0028, profile 1.3.0
   `display`/`sessionQuality`, `validate_for_device`).
5. Tracker: status of L.0, L.1, X.0c, M.1; migration rule line (0028 = L.0; B.21 → 0029 if still unapplied).

## Trap list

- `react/jsx-no-literals` with `ignoreProps: false` flags `className`/`href`; use `ignoreProps: true` plus the
  custom rule (I18N-9). It does not see strings outside JSX.
- SwiftUI `Text("literal")` is a `LocalizedStringKey`: after L.2 a catalogued literal is localised; the Swift lint
  needs `--catalog` to know that, otherwise L.2 drowns in false positives.
- `Intl.NumberFormat("ne-NP")` defaults to Devanagari digits; pass `-u-nu-latn` (owner question 12).
- `PediatricAgeBracket` raw values are persisted in the encrypted profile; changing them without the legacy
  decoder silently resets every child to the default bracket.
- A 1.3.0 profile published before devices run X.0c is refused by older apps (DSP-6); the backend publish path
  stays strict on purpose.
- Two console packages editing `eslint.config.mjs` is the one planned overlap; keep each package's change to its
  own commented block.
- React `cache` dedupes per request only when the same function object is used; `[org]/layout.tsx` and
  `resolveLocale` must both call the shared `myProfile()` from `src/lib/i18n/server.ts`.
