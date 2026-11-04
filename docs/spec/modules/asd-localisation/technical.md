# ASD localisation (WP-L.0, WP-L.1) and iOS constants from the bundle (WP-M.1) - Technical Spec

Decision record: `docs/spec/adr/ADR-0013-three-audience-platform-architecture.md` (principle 6 "no hard-coded
parameters", principle 7 "every user-facing string is localised"); the parameter registry it builds on is
ADR-0012 / `docs/spec/modules/asd-research-toolkit/technical.md` (PRM-*, INV-*). Claims: ADR-0004. Work packages:
`work-packages.md` in this folder; acceptance scenarios: `acceptance.md`. Written 2026-11-04 against `dev` at
`e75053f` (phase 8 waves 0–3 merged).

Ids here: **I18N** (catalogue, tooling, locale selection, console), **DSP** (display bands and iOS constants,
the WP-X.0c contract addition and WP-M.1). Every MUST is testable; the proving test or command is named per
package in `work-packages.md`.

## Purpose and scope

In:
- **WP-L.0** the shared domain vocabulary catalogue `asd_contract::i18n` (`en`, `ne`; optional partial `hi`),
  its JSON export `docs/spec/schemas/asd/i18n-v1.json`, the generated console module
  `apps/curv_console/src/messages/domain.ts`, and `asd-cli i18n export|check|lint`.
- **WP-L.1** the console fully in `ne`: a `ne` tree for every `src/messages/*.ts`, locale per org default and per
  user override (migration **0028**), negotiation user > org > `Accept-Language` > `en`, `<html lang>`, localised
  formatting, domain enums rendered from the catalogue, problem slugs rendered from the catalogue, the Devanagari
  web font, and the literal-string gate (ESLint `react/jsx-no-literals` + an aria/props rule) failing in CI.
- **WP-X.0c + WP-M.1** the iOS traffic-light cut-offs, age splits and remaining product constants moved into the
  registry and the signed bundle (`ScoringProfile` 1.3.0 `display` and `sessionQuality` sections), read on the
  device through `asd-ffi`, with the band decision in Rust and a grep gate in `scripts/`.

Out (deferred, with the package that owns it):
- **Server-rendered text** - `Problem.detail`, `ProfileProblem.message`, export README/manifest text, CSV data
  dictionary, `ExportRequest.locale`: **WP-L.4**. Until then the console renders the catalogue text for the
  problem **slug** (I18N-19) and shows the English `detail` only as a secondary line; the registry form keeps the
  English Rust validation message (it names the path and the range, which read the same in both locales).
- **PDFs and letters** (clinical summary PDF WP-I.5, letters WP-C.12): English with the base-14 fonts until the
  Devanagari font decision (ADR-0008 Q3 / ADR-0013 Q7) and WP-I.5; then WP-L.4 adds the `locale` argument.
- **iOS String Catalog** and its ~360 literal call sites: **WP-L.2** (L.0 only provides the `xcstrings` export
  target and the Swift lint with its baseline). WP-M.1 adds no new hard-coded English: its few new strings go
  through `AppLocalizer` (en/ne/hi), which L.2 folds into the catalog.
- **Android strings**: **WP-L.5** (L.0 provides the `android` target and the Kotlin lint).
- **Nepali review** by a native-speaking clinician: **WP-L.3**. Every `ne` text written by L.0/L.1 is a reviewed
  machine draft marked `// ne: draft` (console) or listed in `i18n-v1.json` `draft: true` (catalogue).

## Verified current state this spec builds on

- **Registry.** `asd_contract::params` (88 rows, `REGISTRY_VERSION = "1.0.0"`) gives each row `label_key =
  "param.<id>.label"`, `help_key = "param.<id>.help"` (PRM-5). The texts live today in
  `apps/curv_console/src/messages/research.ts` (`params` blocks for `en` and `ne`, maintained by
  `apps/curv_console/scripts/gen-registry-messages.mjs` from `param-registry-v1.json`), not in Rust. `ageBands`
  is one registry leaf of kind `AgeBands` (`AgeBracket { id, minMonths, maxMonths }`, default ids `12-17`,
  `18-23`, `24-35`, `36-47`, `48-60`). `params::unknown_keys` reports **any** unregistered top-level key as
  `<path>: unknown parameter`, and `asd_sync::accept_bundle` runs the same checks on the device.
- **There is no `display` section.** ADR-0013 and INV-1 name `display.bands.*` "delivered by profile 1.2.0"; it
  was not built in WP-X.0. WP-X.0c below adds it.
- **Console messages.** `src/messages/{en,admin,catalog,clinical,config,exports,participant,privacy,security}.ts`
  are English only (plain objects, imported statically as `en`, `adminText`, …: 33 + 9 + 14 + 6 + 18 + 7 + 10 +
  8 + 4 import sites). `research.ts` alone has `en` + `ne` (`ResearchMessages` interface, `researchMessages = {
  en, ne }`) but every page imports `researchText = en`. No locale provider, no locale switch, `<html lang="en">`
  fixed in `src/app/layout.tsx`; `src/lib/format.ts` formats dates as fixed UTC ISO (locale-neutral) and ages as
  `5m`/`2h 30m`/`3d 4h` (English unit letters). Domain enums are mapped in the message files
  (`participant.ts` `encounterKinds`, `attachmentKinds`; `clinical.ts` `cosignPolicies`, …) with a few raw leaks
  (`language`, `age_bracket_at_enrol`, `capture_source`, job `error`). ESLint: `eslint-config-next`
  core-web-vitals + typescript; `eslint-plugin-react` is present through it; no literal rule.
- **Backend.** `organizations.settings jsonb` (non-secret keys, `OrgPatch.settings` merged, `OrgUpdate` =
  owner/admin); `users` has no locale; `UserProfile` (`GET/PATCH /v1/users/me`) has no locale. Highest
  migration: **0026** (`0026_research_toolkit.sql`); **0027 is reserved for WP-B.21** (blocked on ADR-0011 Q3).
- **Problems.** `asd_contract::problem` has 67 slug constants; the console maps a few slugs to messages
  (`clinicalErrorMessage`, `shareErrorKey`) and otherwise shows `title`/`detail`.
- **Rust-produced caregiver text.** `asd_core::flag::assess` returns five fixed English
  `headline`/`detail` pairs (no scored clips; unreliable; matches; differs; some differ) that the iOS app writes
  into the session and the console shows as sent.
- **iOS.** `apps/ios_asd_gaze_app/Sources/UI/AppLocalizer.swift` (`AppLocale` en/ne/hi, `AppLocalizer.tr(key,
  lang:)`, 143 keys); no `.xcstrings`; `project.yml` `developmentLanguage: en`; ~220 `Text("…")` and ~144 other
  literal calls. The claims scanner `scripts/check_asd_claims.py --swift` already scans UI-call literals for
  forbidden terms (CLM-2). The device reads the bundle as `ScoringConfiguration` (`scoringProfileJSON`, from
  `asd-ffi` `AsdConfigBundle.scoring_profile_json` or `defaultScoringProfileJson()` for the bundled default).
  `PediatricAgeBracket` raw values are display labels (`"12–17 mo"`), persisted in the encrypted
  `ChildProfile`; `ageMonths` sent with a session is `midpointMonths` (14, 20, 28, 40, 54).

## 1. Domain vocabulary catalogue (`asd_contract::i18n`) - WP-L.0

- **I18N-1 Module, files, keys.** New module `rust/crates/asd-contract/src/i18n.rs` with the catalogue files
  `rust/crates/asd-contract/i18n/domain.en.json`, `domain.ne.json` and optional `domain.hi.json`, compiled in with
  `include_str!` and parsed once (`OnceLock`). Each file is a flat JSON object `key → string`, sorted by key
  (written by `asd-cli i18n fmt`, checked by `i18n check`). Key grammar: `^[a-z][a-z0-9_]*(\.[A-Za-z0-9_-]+)+$`;
  the first segment is the **group**. Placeholders are `{name}` (ASCII identifier), no plural engine: a count
  that needs a plural gets two keys (`…one`, `…other`) and the surface picks with `Intl.PluralRules` / Foundation.
  Types:
  ```rust
  pub enum Locale { En, Ne }            // serde "en" | "ne"; the console and backend locales
  pub enum CatalogueLocale { En, Ne, Hi }  // catalogue files; hi is partial, falls back to en
  pub fn text(locale: CatalogueLocale, key: &str) -> Option<&'static str>;
  pub fn render(locale: CatalogueLocale, key: &str, args: &[(&str, &str)]) -> Option<String>;
  pub fn catalogue() -> &'static Catalogue;   // { version, entries: BTreeMap<key, Entry{ en, ne, hi?, params, draft }> }
  pub const CATALOGUE_VERSION: &str = "1.0.0"; // bumped when a key is removed or changes meaning
  pub fn required_keys() -> BTreeSet<String>;  // I18N-2
  pub fn problems() -> Vec<CatalogueProblem>;  // I18N-3, { key, locale, rule, message }
  ```
  `render` is for Rust code that must produce final text (today: none at runtime; WP-L.4 uses it). Surfaces never
  call Rust to translate UI text at runtime; they use the generated artefacts (I18N-6/7).
- **I18N-2 What is in the catalogue (groups).** The catalogue holds **domain vocabulary that crosses surfaces**,
  never UI chrome (buttons, page titles, form hints stay in each surface's own catalogue). `required_keys()` is
  computed from Rust, so a new enum value or registry row without text fails `cargo test -p asd-contract`:

  | Group (key prefix) | Source of the required keys | Keys |
  | --- | --- | --- |
  | `encounter_kind.<v>`, `encounter_status.<v>` | `emr::EncounterKind::ALL`, `EncounterStatus::ALL` | 5 + 2 |
  | `attachment_kind.<v>` | `emr::AttachmentKind::ALL` | per enum |
  | `care_event_kind.<v>` | `care::CareEventKind` (task, medication, diary, milestone); the caregiver's care-task categories on iOS map to these four | 4 |
  | `role.<v>`, `user_status.<v>`, `org_kind.<v>` | `api_v1::Role`, `UserStatus`, `OrgKind` | per enum |
  | `review_reason.<v>`, `review_status.<v>` | `api_v1::ReviewReason`, `ReviewStatus` | per enum |
  | `clinical_review_state.<v>`, `disposition.<v>`, `cosign_policy.<v>`, `patient_ref_mode.<v>`, `clinical_note_kind.<v>`, `letter_kind.<v>`, `letter_status.<v>`, `timeline_entry_kind.<v>` | `clinical::*::ALL` | per enum |
  | `attention_flag.<v>` | `asd_core::types::AttentionFlag` (green, yellow, red) and `api_v1::SessionAttentionFlag` | per enum |
  | `flag_assessment.<case>.headline` / `.detail` | `asd_core::flag::FlagCase` (new, I18N-4): `no_scored_clips`, `unreliable`, `matches`, `differs`, `some_differ` | 10 |
  | `expectation_status.<v>`, `quality_verdict.<v>`, `tracking_verdict.<v>`, `calibration_quality.<v>`, `scoring_status.<v>`, `capture_source.<v>` | `asd_core::types::*` and the session 1.8 `scoringStatus` | per enum |
  | `export_format.<v>`, `export_mode.<v>`, `export_purpose.<v>`, `export_status.<v>`, `dataset_format.<v>` | `export::*::ALL` | per enum |
  | `experiment_status.<v>`, `run_kind.<v>`, `run_status.<v>`, `level.<v>` | `research::*` | per enum |
  | `config_document_kind.<v>`, `config_document_status.<v>` | `api_v1::*` | per enum |
  | `param.<id>.label`, `param.<id>.help` | `params::registry()` (`label_key`, `help_key`) | 2 × rows |
  | `param_group.<segment>` | first path segment of every registry id | per group |
  | `unit.<unit>` | `params::Unit` (wire spelling) | per unit |
  | `age_band.range` (`{min}–{max} mo`), `age_band.range_open` (`{min}+ mo`) | fixed | 2 |
  | `problem.<slug>` | every slug constant in `problem.rs` (exposed as `problem::ALL_SLUGS`, new) | 67 |
  | `display_band.<status>` (`ok`, `monitor`, `outside`, `not_observed`) and `display_metric.<metric>` | `DisplayMetric::ALL` (WP-X.0c, DSP-3) | added by X.0c |

  **Problem slugs are in**: `problem.<slug>` is one short user-facing sentence per slug (what happened and what
  to do), the text the console shows instead of the server `title`. The free-form server `detail` is not in the
  catalogue (WP-L.4 turns it into `{slug, params}`). Enum **wire values never change**; a display label is always
  a key lookup.
- **I18N-3 Catalogue checks** (`i18n::problems()`, run by `cargo test -p asd-contract` and `asd-cli i18n check`):
  1. every required key has a non-empty `en` and `ne` value; `hi` may be absent (falls back to `en`);
  2. no orphan: every key in `domain.en.json` is required, or starts with a prefix in `FREE_GROUPS` (empty in
     L.0; a group that is not derived from Rust must be listed there with a comment);
  3. `domain.ne.json` and `domain.hi.json` have no key that `en` lacks;
  4. the placeholder set of every value equals the `en` value's set;
  5. every `ne` value contains at least one Devanagari code point (U+0900–U+097F), except keys listed in
     `LATIN_OK` (units such as `unit.ms`, format names `export_format.csv`, `export_format.fhir_r4`);
  6. no value has leading/trailing whitespace, a double space, or a straight ASCII quote where a typographic one is
     meant (`'`/`"` inside `en` prose is a finding; the catalogue uses ’ “ ”);
  7. no `en` value contains a term of `asd_contract::claims::DENY_TERMS` (new, a Rust copy of the claims
     scanner's `GLOBAL_DENY` list; `asd-cli`'s test parses the Python list as text and asserts the two are equal,
     so the Python file stays untouched and the Rust list is the start of the WP-D.8 port);
  8. files are sorted and pretty-printed exactly as `i18n fmt` writes them.
- **I18N-4 One source for Rust-produced caregiver text.** `asd_core::flag` gains `pub enum FlagCase { NoScoredClips,
  Unreliable, Matches, Differs, SomeDiffer }`, `FlagCase::key(self) -> &'static str` (`"flag_assessment.matches"`
  …) and `pub fn flag_case(a: &AttentionFlagAssessment) -> Option<FlagCase>` (matches the English headline
  constants; `None` for text from another version). `assess` keeps its output byte-for-byte (goldens unchanged).
  A test in `asd-contract` asserts that each `FlagCase` headline/detail constant equals the catalogue `en` value,
  so the English text exists once in practice and the catalogue cannot drift from what devices write. `asd-wasm`
  exports `flagCase(headline) -> string | null` so the console renders `flag_assessment.<case>` in the user's
  locale instead of the sent English.
- **I18N-5 Export document.** `asd-cli schema export` writes `docs/spec/schemas/asd/i18n-v1.json`:
  `{"$id": "https://schemas.curv.org/asd/i18n-v1.json", "catalogueVersion": "1.0.0", "locales": ["en","ne"],
  "partialLocales": ["hi"], "entries": {"<key>": {"en": "…", "ne": "…", "hi": "…"?, "params": ["min","max"],
  "draft": true?}}}` (sorted keys, JCS-stable). `schema check` fails on drift like every other schema. The file is
  the contract other surfaces and translators read; WP-L.3's reviewer works from it and clears `draft`.
- **I18N-6 Console module (generated).** `asd-cli i18n export --target console --out
  apps/curv_console/src/messages/domain.ts` writes (header `// GENERATED by asd-cli i18n export; do not edit`):
  ```ts
  export type DomainLocale = "en" | "ne";
  export type DomainKey = "encounter_kind.screening_visit" | …;          // union of every key
  export type DomainParams = { "age_band.range": { min: string | number; max: string | number }; … };
  export const DOMAIN: Record<DomainLocale, Record<DomainKey, string>>;
  export function dt<K extends DomainKey>(locale: DomainLocale, key: K, ...args: K extends keyof DomainParams ? [DomainParams[K]] : []): string;
  export function enumLabel(locale: DomainLocale, group: DomainGroup, value: string): string; // unknown value → the raw value wrapped in ‹ ›, never throws
  ```
  `dt` is a lookup plus `{name}` substitution, nothing else (no plural or number logic in generated code). `i18n
  check` regenerates in memory and fails when the committed file differs (`--console PATH`, default the path
  above). `apps/curv_console/scripts/gen-registry-messages.mjs` and the `params` blocks of `research.ts` are
  retired by L.0: the registry form reads `dt(locale, spec.label_key)`.
- **I18N-7 Native targets (generated, committed later).** `--target xcstrings --out FILE` writes a String Catalog
  (`sourceLanguage: en`, `ne` and `hi` localisations, `hi` only where present, `state: needs_review` for draft
  entries); `--target android --out DIR` writes `values/domain_strings.xml`, `values-ne/…`, `values-hi/…` (keys
  with `.` → `__`, `{name}` → `%1$s` positional in parameter order, XML-escaped, apostrophes escaped). L.0 tests
  both against goldens; WP-L.2 commits `Domain.xcstrings`, WP-L.5 the XML. `i18n check --xcstrings FILE
  --android DIR` adds them to the drift check once they exist.

## 2. Literal-string lint (`asd-cli i18n lint`, ESLint) - WP-L.0 tooling, WP-L.1 console gate

- **I18N-8 Swift and Kotlin lint.** `asd-cli i18n lint [--swift DIR]… [--kotlin DIR]… [--exclude GLOB]… --baseline
  FILE [--catalog Localizable.xcstrings] [--update-baseline] [--format text|json]`.
  - Swift findings: a string literal containing a letter as the first argument of `Text(`, `Label(`,
    `Button(`, `Toggle(`, `Picker(`, `Section(`, `Link(`, `TextField(`, `SecureField(`, `.navigationTitle(`,
    `.accessibilityLabel(`, `.accessibilityHint(`, `.alert(`, `.confirmationDialog(`, `.help(`, and
    `NSLocalizedString`-free `String(format: "…")` used in those calls. Not findings: `Text(verbatim:)`,
    `String(localized:)`, `AppLocalizer.tr(`, a literal with no letter (`"·"`, `"\(n)"`), lines with
    `// i18n-ok: <reason>`, and - when `--catalog` is given - a SwiftUI literal that **is** a key of that
    String Catalog with a `ne` value (SwiftUI resolves `Text("key")` through `LocalizedStringKey`, so after L.2 a
    catalogued literal is localised, not a leak).
  - Kotlin findings: a literal containing a letter in `Text("…")`, `setText("…")`, `contentDescription = "…"`,
    `title = "…"`, `Toast.makeText(…, "…"`, `label = { Text("…") }`; not findings: `stringResource(`,
    `getString(R.string.`, `// i18n-ok:`.
  - Scanner: a small tokenizer in `asd-cli` (string literals with escapes, `#"…"#` raw strings, multi-line `"""`
    skipped as prose blocks and reported under rule `multiline`, comments stripped), not regex-only, no new crate.
  - **Baseline then fail.** The baseline file `scripts/i18n/literal-baseline.txt` holds one line per accepted
    finding `path<TAB>rule<TAB>sha256(literal)[:12]<TAB>count` (no line numbers, so edits elsewhere do not churn
    it). Exit 0 when every finding is in the baseline; **exit 1 on any finding not in the baseline** (a new
    literal fails CI from L.0 on); exit 1 too when the baseline has entries no longer found (stale: run
    `--update-baseline`), so the baseline only shrinks. `--update-baseline` rewrites it sorted. Exit 2 on usage or
    I/O errors. L.0 generates the first baseline over `apps/ios_asd_gaze_app/Sources`,
    `domains/asd_screening/ios/Sources` and `apps/android_asd_gaze_spike` (expected ≈ 360 Swift entries).
  - CI: `rust.yml` runs `cargo run -p asd-cli -- i18n check` and the lint over those roots; `scripts/ios_test.sh`
    adds the lint as a static gate step (the claims step stays).
- **I18N-9 Console gate (ESLint).** `apps/curv_console/eslint.config.mjs` adds, for `src/**/*.tsx`:
  - `react/jsx-no-literals`: `["error", { noStrings: true, ignoreProps: true, allowedStrings: ["…", "·", "—",
    "–", "/", "|", ":", "(", ")", "%", "×", "+", "-", "*"] }]`. Note: ADR-0013 wrote `ignoreProps: false`; that
    flags every `className`, `href` and `type`, so the props half is the custom rule below instead.
  - `curv/no-literal-ui-props` (local rule in `apps/curv_console/eslint-rules/no-literal-ui-props.mjs`, loaded as
    an inline plugin, no new dependency): a string literal or an expression-free template literal containing a
    letter in the JSX attributes `aria-label`, `aria-description`, `aria-placeholder`, `aria-roledescription`,
    `aria-valuetext`, `title`, `placeholder`, `alt`, `label` and in `metadata.title`/`description` objects of
    `page.tsx`/`layout.tsx`. Its test file `eslint-rules/no-literal-ui-props.test.mjs` uses ESLint's `RuleTester`.
  - Exemptions: `// eslint-disable-next-line react/jsx-no-literals -- i18n-ok: <reason>` only (reason required,
    `eslint-comments` style is enforced by review, not by a plugin); test files (`**/*.test.tsx`, `e2e/**`) and
    the generated `domain.ts` are excluded.
  - Ratchet: step 0 sets both rules to `"warn"` globally and lists the directories already clean under an
    `"error"` override block (`LITERAL_GATE_ERROR_DIRS`); each L.1 package appends its directories when they are
    clean; the integration commit after the wave replaces the list with `"error"` everywhere. `npm run lint` uses
    `--max-warnings 0` from the integration commit on.
  - Strings in non-JSX TypeScript (a `label: "Yes"` in a `.ts` helper) are not caught by these rules; the
    message-file tests (I18N-15) and the e2e Nepali smoke (acceptance 9) cover them, and code review rejects them.

## 3. Locale selection - WP-L.1 (contract and migration in step 0)

- **I18N-10 Migration 0028** `rust/crates/asd-backend/migrations/0028_locale.sql` (one file, owner step 0):
  ```sql
  ALTER TABLE users ADD COLUMN locale text NULL CHECK (locale IN ('en', 'ne'));
  ALTER TABLE organizations ADD COLUMN default_locale text NOT NULL DEFAULT 'en' CHECK (default_locale IN ('en', 'ne'));
  -- carry an existing settings.locale (none expected) into the column, then drop the key
  UPDATE organizations SET default_locale = settings->>'locale'
   WHERE settings->>'locale' IN ('en', 'ne');
  UPDATE organizations SET settings = settings - 'locale' WHERE settings ? 'locale';
  ```
  Columns, not the JSONB `settings.locale` that ADR-0013's consequences mention: typed, CHECK-constrained, readable
  by the login query without JSON, and `settings` stays for free-form keys (owner question 11 records the
  choice). `PATCH /orgs/{id}` refuses `settings.locale` with `400 validation-failed` pointing at `default_locale`.
  **0027 stays reserved for WP-B.21**; 0028 may be applied before 0027 exists only because the runner applies
  by number and B.21 is blocked: if B.21 is unblocked before L.1 lands, B.21 keeps 0027 and nothing changes; if
  L.1 lands first, **B.21 must take 0029** (a reserved number below an applied one is the out-of-order case of
  the migration rule). The tracker rule is updated to say so.
- **I18N-11 Contract** (step 0, `asd-contract`, `api-v1.json` regenerated):
  - `UserProfile.locale: Option<Locale>` (the override, `null` = follow the org) and `UserProfile.effective_locale:
    Locale` (I18N-12 result for this request); both `serde(default)` (an older backend yields `None`/`en`).
  - `UpdateProfileRequest.locale: Option<Option<Locale>>` (absent = unchanged, `null` = clear the override,
    `explicit_null` deserializer as for `research_frames_retention_months`).
  - `Org.default_locale: Option<Locale>` (present on `GET /orgs/{id}`) and `OrgPatch.default_locale:
    Option<Locale>` (`OrgUpdate`: owner, admin; the existing org-settings audit diff records it as
    `default_locale`).
  - `i18n::negotiate(user: Option<Locale>, org: Option<Locale>, accept_language: Option<&str>) -> Locale`.
- **I18N-12 Negotiation order: user > org > `Accept-Language` > `en`.** `negotiate` returns the user's override
  if set, else the org default (always set after 0028, so `Accept-Language` matters only where no org applies:
  `superadmin` without a home org, the login page, the public share page), else the best `Accept-Language` match
  (RFC 9110 weighted list, `q=0` excluded, primary subtag match: `ne`, `ne-NP` → `ne`; `en-*` → `en`; anything
  else ignored; ties keep header order), else `en`. The cases are a fixture exported by `asd-cli schema export`
  as `docs/spec/schemas/asd/i18n-negotiation-cases.json` (`[{user, org, acceptLanguage, expected}]`, ≥ 20 rows);
  the Rust test and the console's TS test both run it, so the console's no-session fallback (I18N-16) cannot
  diverge. The backend computes `effective_locale` on `GET/PATCH /users/me` from the stored values and the
  request's `Accept-Language` (the console's live client forwards the browser header).
- **I18N-13** Backend (wave): `GET /users/me` returns `locale`, `effective_locale`; `PATCH /users/me` sets or
  clears `locale` (audit `user.profile_update` with `{"locale": [old, new]}`); `GET/PATCH /orgs/{id}` read and
  write `default_locale`; the login response is unchanged (the console asks `/users/me` next anyway). No other
  backend text changes in L.1.

## 4. Console in Nepali - WP-L.1

- **I18N-14 Locales.** The console ships `en` and `ne` (`Locale`). `hi` is not a console locale.
- **I18N-15 Message trees.** Every `src/messages/<file>.ts` exports `<file>Messages = { en, ne } as const` where
  `ne` is typed by the `en` tree (`const ne: Widen<typeof en> = {…}`; `Widen` in `src/lib/i18n/types.ts` maps
  string literals to `string` and keeps function signatures), so a missing or extra key is a type error.
  Function-valued messages (`(seconds: number) => string`) exist in both trees with the same signature. Step 0
  converts each file to this shape with `ne` = a typed **copy of `en` marked `// ne: TODO`**; the owning wave
  package replaces it with the Nepali draft (`// ne: draft` on the object), and its test (`tests/messages.test.ts`,
  one `describe` per file) asserts: same key set; no `ne` string equal to its `en` string except keys listed in
  the file's `NE_SAME_AS_EN` (brand names, units, codes such as `MRN`, `TOTP`, `CSV`); every other `ne` string
  contains Devanagari; placeholders (`${…}` arity of functions, `{name}` in strings) unchanged; no `// ne: TODO`
  left. The existing research `ne` tree keeps its shape and loses only the `params` blocks (now `domain.ts`).
- **I18N-16 Resolution and access.**
  - `src/lib/i18n/server.ts`: `resolveLocale = cache(async () => …)`: with a session, `myProfile()` (a `cache`d
    `api.getMyProfile()` shared with `[org]/layout.tsx`, so one `/users/me` per request) `.effective_locale`;
    without a session (login, `/share/*`, not-found) `matchAcceptLanguage(headers().get("accept-language"))`
    (`src/lib/i18n/negotiate.ts`, tested against `i18n-negotiation-cases.json` with `user`/`org` null), else
    `en`. `getText = cache(async () => messagesFor(await resolveLocale()))` returns the whole bundle `{ app,
    admin, catalog, clinical, config, exports, participant, privacy, research, security, domain }` for the locale.
  - `src/lib/i18n/messages.ts`: `messagesFor(locale)` assembles the bundle from each file's `{en, ne}`.
  - `src/lib/i18n/provider.tsx` (`"use client"`): `LocaleProvider({ locale })` in the root layout; `useLocale()`
    and `useText()` (= `messagesFor(useLocale())`) for client components. Both locales are in the client bundle
    (≈ 2 × 3,600 lines of strings; accepted, no dynamic import, CSP unchanged).
  - `src/app/layout.tsx`: `<html lang={locale === "ne" ? "ne-NP" : "en"}>`, `metadata` title from the bundle
    (`generateMetadata`).
  - Server components replace `import { en } from "@/messages/en"` (and `adminText`, `researchText`, …) with
    `const t = await getText()` and `t.app.…`/`t.admin.…`; client components with `const t = useText()`. A
    component that receives text through props keeps doing so. After L.1 no file under `src/app` or
    `src/components` imports a message file directly (an ESLint `no-restricted-imports` pattern
    `@/messages/*` except `@/messages/domain` types, from the integration commit).
- **I18N-17 Formatting.** `src/lib/format.ts` functions take the locale: `formatNumber(v, digits, locale)`,
  `formatPercent(v, locale)` and `formatAge(iso, now, locale)` use `Intl.NumberFormat(tag)` with `tag = "en-GB"` or
  `"ne-NP-u-nu-latn"` (Latin digits; owner question 12) and the unit words from the bundle (`t.app.units.minutesShort`
  …); `formatDateTime` stays the fixed UTC ISO form (locale-neutral, no hydration drift), the Gregorian calendar
  only (owner question 13). Plurals use `Intl.PluralRules(tag)` with `one`/`other` keys.
- **I18N-18 Domain labels.** Every enum shown to a user is rendered with `enumLabel(locale, group, value)` or a
  `dt` key from `domain.ts`; the per-file maps (`participant.ts` `encounterKinds`, `attachmentKinds`,
  `clinical.ts` `cosignPolicies`, `patientRefModes`, session/review status maps, export format maps, …) are
  deleted in favour of the catalogue. The known raw leaks are fixed: participant `language` (an ISO code →
  `Intl.DisplayNames(tag, { type: "language" })`), `age_bracket_at_enrol` (band id → `age_band.range` with the
  band's months from the effective profile, else the raw id), `capture_source` (`capture_source.<v>`), job
  `error` (shown under a localised "Error detail" label, content as sent). Flag headline/detail: `flag_assessment.
  <flagCase(headline)>` (I18N-4), falling back to the sent text when `flagCase` is `null`.
- **I18N-19 Problems.** `src/lib/api/problem.ts` (`problemText(t, problem)`) returns `dt(locale,
  "problem.<slug>")` when the slug is known, else the server `title`; the server `detail` is shown only as a
  secondary, smaller line under a localised "Server detail" label when it adds information (it is English until
  WP-L.4). `clinicalErrorMessage` and `shareErrorKey` are folded into it.
- **I18N-20 Settings UI.** `/[org]/settings/security` (the user's own settings page) gains a "Language" select
  (`System default (organisation: …)`, `English`, `नेपाली`), saved with `PATCH /users/me {locale}` through a server
  action that then `revalidatePath("/", "layout")`; `/[org]/settings/clinical` (org settings, `OrgUpdate`) gains
  "Default language for this organisation" (`PATCH /orgs/{id} {default_locale}`). Both are visible to every role
  for the user setting and to owner/admin for the org setting (existing permissions; no new RBAC action).
- **I18N-21 Devanagari web font.** The console self-hosts **Noto Sans Devanagari** (SIL OFL-1.1, Google Noto
  project; variable weight 400–700, `woff2`, subset to Devanagari + Latin + punctuation, ≈ 120 KiB) under
  `apps/curv_console/src/app/fonts/` with its `OFL.txt`, loaded by `next/font/local` (no third-party request; CSP
  `font-src 'self'` unchanged) and applied as `:lang(ne) { font-family: var(--font-devanagari), system-ui, … }`.
  The README "Licences" section lists it. This is the default pending owner ratification (ADR-0013 Q7 / ADR-0008
  Q3, which also unblocks PDFs in WP-I.5). iOS uses the system Devanagari font (Kohinoor Devanagari; nothing
  bundled); Android uses the system Noto.
- **I18N-22 Public and unauthenticated pages.** Login and `/share/*` use `Accept-Language` only (I18N-16); the
  share page's mandatory English sentences ("This is not a diagnostic tool.") stay English and appear in addition
  to the localised text (ADR-0004 d5).
- **I18N-23 Claims.** The ADR-0004 mandatory sentences are rendered in the user's locale **and** in English on
  every result surface in the console (`ne` users see both lines); the console's claims check (existing
  `tests/claims.test.ts` or equivalent) runs over both trees.

## 5. Display bands and iOS constants (DSP) - WP-X.0c contract addition, WP-M.1 iOS

### 5.1 Inventory (WP-M.1, grep of `domains/asd_screening/ios/Sources/**` and `apps/ios_asd_gaze_app/Sources/**` at `e75053f`)

Legend for the verdict column: **row** = an existing registry id; **new** = a WP-X.0c row (§5.2); **core** = the
decision comes from a Rust result the device already has (no threshold in Swift); **exempt** = not a product
parameter, listed in the gate's allowlist with the reason.

| # | file:line | Constant | Today | Maps to |
| --- | --- | --- | --- | --- |
| 1 | `apps/…/Views/PerVideoResultViews.swift:584-585` | ring colour of a **composite** score (`ScoreRingView`, call sites :141, :233) | ≥ 0.65 on target, ≥ 0.35 monitor | **core**: the session's `AttentionFlag` from `assess_flag_json` (thresholds `flag.greenFloor`/`flag.yellowFloor`); the ring takes a status, not a score cut-off |
| 2 | `apps/…/Views/PerVideoResultViews.swift:584-585` | ring colour of a **per-clip** subscore (call sites :279, :375) | same 0.65/0.35 | **core**: the clip's `ExpectationStatus` already in the per-video report (`report.status`) |
| 3 | `apps/…/Models/DoctorReportGenerator.swift:318-319` | per-clip status text in the PDF | `..<0.35` outside, `..<0.65` near edge | **core**: `video.status` (`ExpectationStatus`) → `expectation_status.<v>` text |
| 4 | `apps/…/Views/EngagementRiskStratificationView.swift:160-161` (+ target text :169) | social preference % | ≥ 60 ok, 50–60 monitor | **new** `display.bands.socialPreferencePct.okMin` (60), `.monitorMin` (50) |
| 5 | `apps/…/Views/EngagementDetailViews.swift:392-393` (+ :469) | social preference % | ≥ 60 / 50 | same rows as #4 |
| 6 | `apps/…/Views/EngagementDetailViews.swift:482-483` (+ :480) | non-social dwell % | ≤ 40 | derived: `100 − display.bands.socialPreferencePct.okMin` (same band, metric `nonSocialPct`, DSP-3) |
| 7 | `apps/…/Views/CaregiverDashboardView.swift:954` | "Target: >50%" on the weekly social-attention mean | 50 | `display.bands.socialPreferencePct.okMin` (owner question 16: one cut-off per metric; default 60) |
| 8 | `apps/…/Views/EngagementRiskStratificationView.swift:179-180` (+ :188) | eye-region dwell % | ≥ 25 ok, 18–25 monitor | **new** `display.bands.eyeDwellPct.okMin` (25), `.monitorMin` (18) |
| 9 | `apps/…/Views/EngagementDetailViews.swift:495-496` (+ :493) | eye-region dwell % | ≥ 25 / ≥ 18 | same rows as #8 |
| 10 | `apps/…/Views/EngagementDetailViews.swift:508-509` (+ :506) | mouth dwell % | 10–30 within | **new** `display.bands.mouthDwellPct.okMin` (10), `.okMax` (30) |
| 11 | `apps/…/Views/EngagementRiskStratificationView.swift:198-199` (+ :207) | first-eye latency ms | < 550 ok, 550–750 monitor | **new** `display.bands.firstEyeLatencyMs.okMax` (550), `.monitorMax` (750) |
| 12 | `apps/…/Views/EngagementDetailViews.swift:440-441, 521-522` (+ :519) | first-eye latency ms | ≤ 650 quick | same rows as #11 (owner question 16: the 650 ms variant adopts 550/750) |
| 13 | `apps/…/Views/EngagementRiskStratificationView.swift:217-219` (+ :225) | smile intensity % | ≥ 20 ok, ≥ 10 monitor | **new** `display.bands.smileIntensityPct.okMin` (20), `.monitorMin` (10) |
| 14 | `apps/…/Views/EngagementAffectAndQualityViews.swift:27-29, 105` (+ :103) | smile intensity % | ≥ 20 / ≥ 10 | same rows as #13 |
| 15 | `apps/…/Views/EngagementAffectAndQualityViews.swift:116-117` (+ :114) | smile reactivity index | ≥ 0.30 | **new** `display.bands.smileReactivity.okMin` (0.30) |
| 16 | `apps/…/Views/EngagementAffectAndQualityViews.swift:128-129` (+ :126) | surprise % | ≥ 15 | **new** `display.bands.surprisePct.okMin` (15) |
| 17 | `apps/…/Views/EngagementAffectAndQualityViews.swift:139-140` (+ :137) | flat-affect score | ≤ 0.25 | **new** `display.bands.flatAffect.okMax` (0.25) |
| 18 | `apps/…/Views/EngagementRiskStratificationView.swift:234` (+ :242) | valid-frame % | ≥ 75 | **new** `sessionQuality.optimalMinValidRatio` (0.75) via metric `validFramePct` (one cut-off with #20) |
| 19 | `apps/…/Views/EngagementAffectAndQualityViews.swift:179` (+ :255) | valid-frame % | ≥ 75 | same as #18 |
| 20 | `domains/…/CurvAsdGazeKit/GazeProcessingActor.swift:1175` | session quality verdict | ≥ 0.75 optimal, ≥ 0.50 acceptable, else degraded; interruption → degraded | **new** `sessionQuality.optimalMinValidRatio` (0.75), `sessionQuality.acceptableMinValidRatio` (0.50); the decision moves to Rust `asd_core::quality::session_verdict` (DSP-4) |
| 21 | `domains/…/CurvAsdGazeKit/GazeProcessingActor.swift:1082, 1143` | `maxFrameDwellSec` | 0.1 s | **row** `aoi.dwellCapMs` (100); read from the active profile (`dwellCapMs / 1000`) |
| 22 | `apps/…/Views/GazeSessionResultView.swift:232` | scanpath line break across gaps | `dt ≤ 0.12` s | **new** `display.scanpathMaxGapMs` (120) |
| 23 | `apps/…/Models/ChildProfile.swift:44-49` | `PediatricAgeBracket` raw values = labels | `"12–17 mo"` … | **row** `ageBands` ids (`12-17` …); labels formatted from `age_band.range` with the band's months (DSP-6) |
| 24 | `apps/…/Models/ChildProfile.swift:73-80` | `midpointMonths` sent as `ageMonths` | 14, 20, 28, 40, 54 | **new** field `ageBands[].representativeMonths` (DSP-2; defaults 14, 20, 28, 40, 54, so sessions carry the same `ageMonths`) |
| 25 | `apps/…/Models/ChildProfile.swift:53-71` | `developmentalStage`, `clinicalFocus` per bracket | English per case | text, not a parameter: keyed by band id in `AppLocalizer` (`age_band.<id>.stage`/`.focus`), shown only for the five default ids, absent for other bands (L.2 moves the keys to the catalog) |
| 26 | `apps/…/Models/OnDemandStimulusManager.swift:91-95` | fallback ODR pack split | `<24`, `24..<36`, else | catalogue document: `StimulusCatalog.agePacks[].minMonths/maxMonths` (already the primary path, `StimulusCatalog.swift:178`); the hard-coded fallback is deleted (no pack → no tag, logged) |
| 27 | `apps/…/Models/ClinicalDashboardViewModel.swift:51` | trend point tag | `duration ≥ 45` s → screen stimuli | not a parameter: the tag comes from the session record (`stimulusSetId != nil` → screen stimuli); the duration guess is deleted |
| 28 | `apps/…/Views/ResearchStudyConsentView.swift:169` | research age stepper range | 12…60 months | **row** `ageBands`: `first.minMonths…last.maxMonths` of the active profile |
| 29 | `domains/…/CurvAsdGazeKit/Inference/EngagementClassificationEngine.swift:145-149, 194-252` | research heuristic: cut-offs 0.35/0.65, weights 0.42/0.24/…, logit (0.38, 6.5), label thresholds | `#if ASD_RESEARCH_CLASSIFIER` only (ADR-0004 d3) | **exempt (research build only)**: excluded from the gate with a pointer to owner question 18 (Swift maths; delete or port, never in Release) |
| 30 | `domains/…/CurvAsdGazeKit/Inference/AgeBandEstimationEngine.swift:36-39, 57-60` | `FacialAgeBand` month ranges | 12…36, 37…72, 73…156, 157… | **exempt**: output classes of the bundled CoreML age model (model-defined, change only with the model); labels to L.2 |
| 31 | `apps/…/Views/EngagementRiskStratificationView.swift` "42% Weight" … | heuristic weights shown as text | research build | **exempt** with #29 |
| 32 | `apps/…/Models/ClinicalDevelopmentModels.swift:34-40` | `CDCSurveillanceAge` 9/12/18/24/30/36 | CDC/AAP milestone schedule | **exempt**: published clinical schedule content, not a tunable; candidate for the catalog document later |
| 33 | `apps/…/Views/MoodDiaryView.swift:157-158` | caption of the caregiver's own intensity slider | `<0.34` mild, `<0.67` moderate | **exempt**: wording of a self-reported slider, not a measurement |
| 34 | `domains/…/CurvAsdGazeKit/GazeProcessingActor.swift:886-890` | simulated affect phases | `ASD_SIMULATION_ENABLED` only | **exempt**: simulation generator (ADR-0001), never in Release |

Totals: **34 sites** (28 product constants, 6 exempt); **16 new registry rows** (§5.2) plus one new `ageBands` field;
5 sites move to Rust results already on the device (`AttentionFlag`, `ExpectationStatus`); 1 existing row
(`aoi.dwellCapMs`) and 1 existing leaf (`ageBands`) newly honoured on iOS. Layout numbers (font sizes, paddings,
`ForEach(0..<n)`, `w > 10` geometry guards), HTTP status ranges, and time-of-day validation (`0...23`) are not
parameters and are not in the inventory.

### 5.2 Registry and contract additions (WP-X.0c)

- **DSP-1 Profile 1.3.0.** `ScoringProfile` gains two `serde(default)` sections and
  `SCORING_PROFILE_SCHEMA_VERSION = "1.3.0"`; a 1.2.0 (or older) body decodes to these defaults; goldens stay
  byte-identical (no scoring input changes). `REGISTRY_VERSION` stays `1.0.0` (rows added, none changed).

  | Registry id | Kind | Unit | Range (step) | Default | Stage | Reprocess | since | Label keys |
  | --- | --- | --- | --- | --- | --- | --- | --- | --- |
  | `display.bands.socialPreferencePct.okMin` | Number | Percent | 0–100 (1) | 60 | Display | Score | 1.3.0 | `param.<id>.label`, `.help` |
  | `display.bands.socialPreferencePct.monitorMin` | Number | Percent | 0–100 (1) | 50 | Display | Score | 1.3.0 | 〃 |
  | `display.bands.eyeDwellPct.okMin` | Number | Percent | 0–100 (1) | 25 | Display | Score | 1.3.0 | 〃 |
  | `display.bands.eyeDwellPct.monitorMin` | Number | Percent | 0–100 (1) | 18 | Display | Score | 1.3.0 | 〃 |
  | `display.bands.mouthDwellPct.okMin` | Number | Percent | 0–100 (1) | 10 | Display | Score | 1.3.0 | 〃 |
  | `display.bands.mouthDwellPct.okMax` | Number | Percent | 0–100 (1) | 30 | Display | Score | 1.3.0 | 〃 |
  | `display.bands.firstEyeLatencyMs.okMax` | Number | Millis | 50–5000 (10) | 550 | Display | Score | 1.3.0 | 〃 |
  | `display.bands.firstEyeLatencyMs.monitorMax` | Number | Millis | 50–5000 (10) | 750 | Display | Score | 1.3.0 | 〃 |
  | `display.bands.smileIntensityPct.okMin` | Number | Percent | 0–100 (1) | 20 | Display | Score | 1.3.0 | 〃 |
  | `display.bands.smileIntensityPct.monitorMin` | Number | Percent | 0–100 (1) | 10 | Display | Score | 1.3.0 | 〃 |
  | `display.bands.smileReactivity.okMin` | Number | Ratio | 0–1 (0.01) | 0.30 | Display | Score | 1.3.0 | 〃 |
  | `display.bands.surprisePct.okMin` | Number | Percent | 0–100 (1) | 15 | Display | Score | 1.3.0 | 〃 |
  | `display.bands.flatAffect.okMax` | Number | Ratio | 0–1 (0.01) | 0.25 | Display | Score | 1.3.0 | 〃 |
  | `display.scanpathMaxGapMs` | Integer | Millis | 16–1000 | 120 | Display | Score | 1.3.0 | 〃 |
  | `sessionQuality.optimalMinValidRatio` | Number | Ratio | 0–1 (0.01) | 0.75 | Metrics | Score | 1.3.0 | 〃 |
  | `sessionQuality.acceptableMinValidRatio` | Number | Ratio | 0–1 (0.01) | 0.50 | Metrics | Score | 1.3.0 | 〃 |

  That is **16 rows** (14 `display.*`, 2 `sessionQuality.*`). `owner_gate: None` for all. Cross-field
  rules in `profile_checks`: `monitorMin ≤ okMin`, `okMax ≤ monitorMax`, `mouthDwellPct.okMin ≤ okMax`,
  `acceptableMinValidRatio ≤ optimalMinValidRatio`. `sessionQuality.*` is `Stage::Metrics` / `Reprocess::Score`:
  it changes the stored `qualityVerdict` of new sessions; the backend's review and norm rules keep reading
  `review.lowQualityValidRatio` (unchanged) and never re-derive a verdict from these rows.
- **DSP-2 `ageBands[].representativeMonths`.** `AgeBracket` (asd-core) gains `representative_months:
  Option<i64>` (`serde(default, skip_serializing_if = "Option::is_none")`, camelCase `representativeMonths`); the
  default profile sets 14, 20, 28, 40, 54 on the five default bands; `params::validate` (kind `AgeBands`) accepts
  the key and checks `minMonths ≤ representativeMonths ≤ maxMonths`; absent means `(minMonths + maxMonths) / 2`
  (integer division). The baselines document's `ageBrackets` ignore it. Golden vectors are unchanged (the
  baselines' brackets carry no value; the profile default is not in a golden).
- **DSP-3 Band decision in Rust.** New `asd_core::display`:
  ```rust
  pub enum DisplayMetric { SocialPreferencePct, NonSocialPct, EyeDwellPct, MouthDwellPct, FirstEyeLatencyMs,
                           SmileIntensityPct, SmileReactivity, SurprisePct, FlatAffect, ValidFramePct }
  pub enum BandStatus { Ok, Monitor, Outside, NotObserved }
  pub struct BandTarget { pub comparator: Comparator /* AtLeast | AtMost | Between */, pub low: Option<f64>, pub high: Option<f64>, pub unit: &'static str }
  pub fn band(metric: DisplayMetric, value: Option<f64>, display: &DisplayParams, quality: &SessionQualityParams) -> (BandStatus, BandTarget);
  ```
  One-sided bands with a `monitor` bound give `Ok`/`Monitor`/`Outside`; one-sided without it give `Ok`/`Outside`;
  `Between` gives `Ok`/`Outside`; `None` → `NotObserved`. `NonSocialPct` uses `100 − socialPreferencePct.okMin`
  as `AtMost`. Boundaries match today's Swift exactly (`≥` for minimums, `<` for latency `okMax`, `≤` for
  `flatAffect.okMax` and the 10–30 range) so the screens do not change colour on the default profile; a table
  test pins every boundary value from §5.1. The contract (`asd_contract::config::{DisplayParams,
  SessionQualityParams}`) converts into the core twins as the other sections do (WP-X.1 pattern).
- **DSP-4 Session verdict in Rust.** `asd_core::quality::session_verdict(valid_ratio: f64, interrupted: bool,
  p: &SessionQualityParams) -> GazeQualityVerdict` reproduces `GazeProcessingActor.swift:1175-1178` (≥ optimal →
  optimal, ≥ acceptable → acceptable, else degraded; any interruption demotes optimal/acceptable to degraded).
- **DSP-5 FFI** (`asd-ffi`, UniFFI records, no JSON for the hot path):
  ```rust
  #[uniffi::export] pub fn display_params_from_profile(profile_json: String) -> Result<AsdDisplayParams, AsdError>;
  #[uniffi::export] pub fn display_band(metric: AsdDisplayMetric, value: Option<f64>, params: AsdDisplayParams) -> AsdBand; // { status, comparator, low, high }
  #[uniffi::export] pub fn session_quality_verdict(valid_ratio: f64, interrupted: bool, params: AsdDisplayParams) -> String; // wire spelling
  #[uniffi::export] pub fn age_bands_from_profile(profile_json: String) -> Result<Vec<AsdAgeBand>, AsdError>; // { id, min_months, max_months, representative_months }
  ```
  `AsdDisplayParams` carries the 16 leaves plus `aoi_dwell_cap_ms` (existing row) so the kit needs one decode per
  configuration. A malformed or pre-1.3.0 profile yields the defaults (never an error for missing sections).
- **DSP-6 Forward-compatible device acceptance.** Because `params::unknown_keys` rejects unknown top-level keys,
  an app built before X.0c would refuse a 1.3.0 bundle (`InvalidProfile`) and keep its cached/bundled profile.
  X.0c adds `params::validate_for_device` (used by `asd_sync::accept_bundle`): unknown keys are **ignored**
  (logged as `unknown_keys: [...]` in the verdict message) while range and type checks still apply to known
  rows; publishing (`config_docs::lifecycle::validate_body`, wasm `validateScoringProfile`) stays strict. Owner
  question 17: no profile containing `display`/`sessionQuality` is published until an app release carrying X.0c
  is on the pilot devices (the backend's `bundle_adoption` table shows it).
- **DSP-7 Schemas.** `config-bundle-v1.json`, `api-v1.json` (profile 1.3.0 `$defs`) and `param-registry-v1.json`
  (88 + 16 = 104 rows) are regenerated; the catalogue gains the 32 `param.display.*`/`param.sessionQuality.*` label/help keys
  plus `param_group.display`, `param_group.sessionQuality`, `display_metric.<m>` (10) and `display_band.<status>`
  (4), en + ne.

### 5.3 iOS (WP-M.1)

- **DSP-8 Reading.** `ScoringConfiguration` (kit) gains `public let display: AsdDisplayParams` and `public let
  ageBands: [AsdAgeBand]`, decoded once in `init` from `scoringProfileJSON` through DSP-5 (bundled default →
  `defaultScoringProfileJson()`; delivered → the bundle's `scoring_profile_json`). A delivered profile without the
  sections gives the defaults (DSP-5), so the fallback chain is: delivered bundle → its defaults for missing
  sections → bundled default profile. Views read `ScoringConfiguration.active.display`; no view stores a copy.
- **DSP-9 Views.** Every site in §5.1 marked **new**/**row** calls `displayBand(metric:value:params:)` (a thin
  Swift wrapper `DisplayBands.status(_:_:)` in the kit) and maps `BandStatus` to the existing colours
  (`ok` → `LiquidGlass.emeraldMint`, `monitor` → `warmAmber`, `outside` → `signalAtypical` where today's view uses
  it, else `warmAmber`; `notObserved` → `.secondary`/`.gray` as today). The "target" strings (`"≥ 60.0%"`, `"<
  550 ms"`, `"10.0% – 30.0%"`) are formatted from `BandTarget` with `NumberFormatter` (current app locale) and
  three `AppLocalizer` keys (`display.target.at_least`, `.at_most`, `.between`; en/ne/hi). Sites marked **core**
  take the status from the Rust result (`AttentionFlag`, `ExpectationStatus`) and contain no comparison.
- **DSP-10 Age bands.** `PediatricAgeBracket` is replaced by `ageBandId: String` in `ChildProfile`; the decoder
  accepts the old raw values (`"12–17 mo"` → `"12-17"`, … via a fixed legacy map that lives only in the migration
  initializer) and the new ids; the encoder writes the id. The picker lists `ScoringConfiguration.active.ageBands`
  formatted with `age_band.range` (`AppLocalizer` key, en/ne/hi) and `ageMonths` for sessions is the band's
  `representativeMonths`. A stored id that the active profile no longer has keeps its last representative months
  (stored alongside: `ageBandRepresentativeMonths: Int`) and the settings screen asks the caregiver to pick again.
  Sessions keep the same `ageMonths` on the default profile (test).
- **DSP-11 Gate.** `scripts/check_ios_constants.sh` (bash + `grep -E`, no Python): fails (exit 1) when any file
  matching `apps/ios_asd_gaze_app/Sources/Views/Engagement*Views.swift`, `…/Views/*Result*View*.swift`,
  `…/Views/EngagementRiskStratificationView.swift`, `…/Views/CaregiverDashboardView.swift`,
  `…/Models/DoctorReportGenerator.swift`, `…/Models/ChildProfile.swift`,
  `…/Models/OnDemandStimulusManager.swift` or `domains/…/CurvAsdGazeKit/GazeProcessingActor.swift` contains a
  **comparison against a numeric literal** (`(>=|<=|>|<|==)\s*-?[0-9]+(\.[0-9]+)?` outside comments, strings and
  `// constants-ok: <reason>` lines), a **`case ..<N` / `N..<M` / `N...M` month or score range**, or a string
  literal of the form `"[<>≥≤] ?[0-9]"`. Layout and loop literals are not comparisons and do not match; the few
  legitimate comparisons (`count > 1`, `dt > 0`) are written against named zero/one constants or carry
  `// constants-ok:`. The allowlist file `scripts/ios-constants-allowlist.txt` names the exempt files (§5.1 #29,
  #30, #32, #33, #34) with the reason. `scripts/ios_test.sh` runs it as the step "Static gate: product constants
  (WP-M.1)"; `--self-test` runs it against fixtures in `scripts/tests/ios_constants/` (a bad file fails, a clean
  one passes).

## Public Rust API sketch (load-bearing names only)

```rust
// asd-contract
pub mod i18n { pub enum Locale; pub enum CatalogueLocale; pub fn catalogue(); pub fn text(); pub fn render();
               pub fn required_keys(); pub fn problems(); pub fn negotiate(); pub const CATALOGUE_VERSION: &str; }
pub mod claims { pub const DENY_TERMS: &[&str]; }
pub mod problem { pub const ALL_SLUGS: &[&str]; }                 // every slug constant, for I18N-2
pub mod config { pub struct DisplayParams; pub struct SessionQualityParams; }  // X.0c, profile 1.3.0
pub mod params { pub fn validate_for_device(..) -> Vec<ProfileProblem>; }       // X.0c, DSP-6
// asd-core
pub mod flag { pub enum FlagCase; pub fn flag_case(..) -> Option<FlagCase>; }   // L.0
pub mod display { pub enum DisplayMetric; pub enum BandStatus; pub struct BandTarget; pub fn band(..); } // X.0c
pub mod quality { pub fn session_verdict(..) -> GazeQualityVerdict; }          // X.0c
// asd-cli
// i18n export --target console|xcstrings|android --out P ; i18n check [--console P] [--xcstrings P] [--android D]
// i18n lint --swift D… --kotlin D… --baseline F [--catalog F] [--update-baseline] ; i18n fmt
```

`asd-wasm` adds `flagCase(headline)` (L.0) and `displayBand`/`sessionQualityVerdict` (X.0c, for parity tests and
a later console use); `asd-ffi` adds DSP-5.
