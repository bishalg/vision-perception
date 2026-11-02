# ADR-0010: Phase 6: Swift 6 language mode, `Biomarker*` → `Engagement*` renames with a wider claims gate, mandatory TOTP per role

Status: Proposed, 2026-11-02
Spec ids: CONC-5, CI-3 (amended), CLM-7, CLM-8 in
`docs/spec/modules/asd-gaze-app-store-readiness/technical.md`; AUTH-11..AUTH-14 in
`docs/spec/modules/asd-rust-core-and-backend/technical.md` section 9
Packages: WP-A.6a/b/c, WP-A.7a/b, WP-B.16, WP-C.17 (tracker `04 §8`); backlog WP-D.8, WP-B.17

## Context

Phase 5 (ADR-0009, `dev` at `03d84a3`) closed the security-operations batch and left three
items for phase 6 by name: WP-A.6 Swift 6 language mode and WP-A.7 the `Biomarker*`
renames (both "tree-wide, cannot run beside any other kit package", D10), and owner
decision O-10, mandatory TOTP per role, which ADR-0009 recommended enforcing "in phase 6
after a 14-day nudge". Q5 asked whether `scripts/check_asd_claims.py` may be extended or
must be ported to `asd-cli` first.

What exists and was measured on 2026-11-02 (Xcode 27.2, Swift 6.4.2, scratch builds only,
nothing committed):

- **Kit** (`domains/asd_screening/ios`, 27 source + 40 test files, tools-version 5.9).
  `swift build --build-tests -Xswiftc -swift-version -Xswiftc 6` and `swift build -c
  release` in the same mode: **0 errors**. The kit already builds with
  `-strict-concurrency=complete -warnings-as-errors` in `scripts/ios_test.sh` (CONC-3), and
  the UniFFI 0.32 bindings (`build/asd/swift/asd_core.swift`, target `AsdCore`) compile in
  Swift 6 mode unchanged. SwiftPM warns "language mode was overridden by extra Swift
  flags", which is why the flip must be in `Package.swift`, not in flags.
- **App** (`apps/ios_asd_gaze_app`, 96 source + 33 test files, `SWIFT_VERSION: "5.9"`,
  `SWIFT_STRICT_CONCURRENCY: complete` in `project.yml`). `xcodebuild build-for-testing
  … SWIFT_VERSION=6.0`: **8 errors in 7 source files**, the widget extension and
  `CareActivityKit` (already tools-version 6.0) clean, about 10 concurrency warnings left
  that stay warnings in Swift 6 (`@preconcurrency` SDK callbacks). The errors are three
  kinds: `static let` of a non-`Sendable` type (`ISO8601DateFormatter` in
  `FHIRInteroperabilityMapper.swift:17`, `KeychainWrapper.standard` in
  `SmartOnFhirAuthService.swift:402`, `[ScreeningWizardStep]` in
  `GazeScreeningWizardView.swift:36`); main-actor statics read from a nonisolated context
  (`DataDeletionService.swift:83,91`); region-isolation "sending … risks causing data
  races" (`ARKitMoodFusionController.swift:32`, `ResearchKitStudyStore.swift:364`). The
  test target could not be compiled in that run (it imports the failed app module); the
  same code under 5.9 + `complete` reports **about 70 diagnostics in 13 test files** that
  become errors in Swift 6, 48 of them in `DataDeletionServiceTests.swift` and
  `CareKitClinicalStoreTests.swift` (captured `var` mutated in a `Sendable` closure,
  main-actor state touched from nonisolated test methods).
- **Renames.** `Biomarker` appears in 8 kit source files, 11 kit test files, 12 app files
  and 4 app test files as the types `SessionBiomarkerAggregator`,
  `BiomarkerClassificationEngine`, `BiomarkerClassificationResult`, `BiomarkerDetailType`,
  `BiomarkerDetailView`, `BiomarkerDetailSheet`, `Biomarker*DetailSection`, the test
  double `LegacySessionBiomarkerAggregator`, and three view files named after them. It
  also appears **on the wire**: `biomarker` is a required property of
  `NormativeExpectation`/`PerVideoScoreRow` in session schemas 1.6, 1.7 and 1.8, in
  `config-bundle-v1.json` (baselines document, Ed25519/JCS signed), in `api-v1.json`
  (console metric rows), in the kit's bundled `normative_baselines.json`, in the vault
  JSON written by `SessionVaultManager` (`"biomarkers"`, `"biomarker"`), in the Rust
  structs that mirror those documents (`asd-core::types`, `asd-contract::mirror`,
  `norms.rs`), and as the persisted raw value `"DigitalBiomarker"` of
  `ClinicalInteroperabilityStandards`. The console already labels the field "Metric"
  (`messages/config.ts`). `docs/research/asd_gaze_biomarkers/` is literature digests where
  the word is the research term.
- **Claims gate.** `check_asd_claims.py --swift` scans only `apps/ios_asd_gaze_app/Sources`
  (not the kit) and only lines that call `Text(`, `Label(`, `Localizer` and the like
  (CLM-2). A read-only prototype of a "prose literal" scan (any string literal with
  whitespace and ≥ 12 characters on a non-UI line) over app and kit sources finds **24
  hits in 15 files** (11 `screening`, 9 `clinical`, 1 each `risk-score`, `hipaa`,
  `diagnos`, `biomarker`), among them the M-CHAT `clinicalRecommendation` texts, the
  research classifier's `"CoreML ANE Multi-Biomarker Synthesis"` and the `AppModeStore`
  mode descriptions. The HANDOFF already named the gap (`ChildProfile.clinicalFocus`,
  `MCHATScoreResult.clinicalRecommendation`).
- **TOTP.** Login (`routes/auth.rs::second_factor`) asks enrolled users for a code and
  passes everyone else; `UserPrincipal::from_request_parts` (`auth/extractors.rs`) reads
  `org_id, role, status` from `users` on every authenticated request and caches the
  principal in the request extensions; `GET /v1/users/me` returns `totp_enrolled` and
  `totp_pending`; the console shows `TotpNudge` for a role list hard-coded in
  `lib/permissions.ts` (`TOTP_NUDGE_ROLES`) on the overview and security pages;
  `[org]/layout.tsx` calls `getOrg` before rendering any page; `proxy.ts` runs on every
  request and already injects `x-nonce`. The iOS app has **no user login**: devices use
  `dvc_` tokens from enrol codes, so O-10 has no iOS surface. Clinical step-up
  (`clinical/reauth.rs`) already requires the code when enrolled. The operator command
  `admin reset-totp` (AUTH-9) is the only recovery; there is no org-admin reset.

## Decisions

1. **Swift 6 language mode is switched on by configuration, not flags, in three
   packages that share no files.** The kit sets `// swift-tools-version: 6.0` in
   `Package.swift` (language mode 6 is the default there; no `swiftLanguageModes`
   override, so the generated `AsdCore` target gets it too). The app sets
   `SWIFT_VERSION: "6.0"` in `project.yml` for every target (the widget's own `5.9`
   setting goes). `scripts/ios_test.sh` keeps its "kit strict concurrency,
   warnings-as-errors" step (it is now a plain `swift build -Xswiftc -warnings-as-errors`;
   `-strict-concurrency=complete` is implied by mode 6 and is dropped from the command to
   avoid the override warning). `SWIFT_TREAT_WARNINGS_AS_ERRORS` is **not** enabled for the
   app in this phase: the warnings that survive mode 6 come from `@preconcurrency` SDK
   callbacks (AVFoundation, SwiftUI `onChange`) and fixing them is not worth a gate. CI-3
   is amended accordingly; CONC-5 is the new MUST. The release-symbol gate is unchanged:
   mode 6 moves no code between configurations, and the gate is re-run on the mode-6
   Release and DemoRelease binaries as the proof.
2. **Concurrency fixes follow the isolation that already exists; no new `@unchecked
   Sendable`.** Non-`Sendable` globals become `let` instances of a `Sendable` type, a
   per-call value, or a `@MainActor` static where every reader is already on the main
   actor; `nonisolated(unsafe)` is allowed only for an immutable value whose type cannot be
   made `Sendable` (`KeychainWrapper.standard` if the wrapper is a class with no mutable
   state; otherwise wrap it in an actor) and must carry a one-line justification. "Sending
   … risks causing data races" is fixed by making the sent value `Sendable` (value types,
   `let` fields) or by moving the consumer onto the producer's isolation, never by
   `Task.detached` tricks. Test code uses `@MainActor` on test classes whose subject is
   main-actor isolated, `await` on actor state, and a small `final class Box<T>:
   @unchecked Sendable` **only** in `Tests/` for captured results, never in `Sources/`.
   CONC-2 (`GazeSessionController` not `@unchecked Sendable`) stands.
3. **WP-A.7 renames Swift identifiers only; every wire name, persisted value, fixture,
   signed document and Rust field keeps `biomarker`.** The JSON property `biomarker` is
   part of three session schemas, the signed config bundle and `api-v1.json`; renaming it
   would need session 1.9, config-bundle v2 with re-signed baselines, a backend upgrade
   path and console changes, for a word the console already displays as "Metric" and
   that no parent ever sees. The Rust structs mirror the documents field-for-field
   (CON-2 "ported 1:1"), so their field names stay too; Rust doc comments that name the
   old Swift types gain the new name in parentheses and nothing else changes in `rust/`.
   The research folder, the persisted `"DigitalBiomarker"` code and the fastlane
   screenshot file names stay. The type map is fixed here so the package is mechanical:
   `SessionBiomarkerAggregator` → `SessionEngagementAggregator`;
   `BiomarkerClassificationEngine` → `EngagementClassificationEngine`;
   `BiomarkerClassificationResult` → `EngagementClassificationResult`;
   `BiomarkerDetailType/View/Sheet` → `EngagementDetailType/View/Sheet`;
   `Biomarker{RiskStratification,FacialAffect,DataQuality}DetailSection` →
   `Engagement…DetailSection`; `LegacySessionBiomarkerAggregator` →
   `LegacySessionEngagementAggregator`; the three view files and five test files are
   renamed with their types (`git mv`); `SessionVaultManager`'s JSON keys and
   `normative_baselines.json` are untouched. Docs that name the types (`docs/spec/**`,
   `docs/ASD_GAZE_TRACKER_ARCHITECTURE.md`, `domains/asd_screening/docs/NORMATIVE_BASELINES.md`,
   the app README, the pitch deck) are updated by the same package; historical ADR text
   is not rewritten (ADR-0004 keeps its quotes).
4. **The claims gate grows a prose-literal mode and covers the kit (Q5: extend the Python
   script; do not port first).** `check_asd_claims.py --swift` scans both
   `apps/ios_asd_gaze_app/Sources` and `domains/asd_screening/ios/Sources/CurvAsdGazeKit`,
   and on every non-comment line, not only UI calls, checks string literals that contain
   whitespace and are at least 12 characters long ("prose literals") against the same
   denylist with the same noisy-rule exemptions (`test`, `astro-fusion`, `study`,
   `ane-facs`) and the same `// claims-ok: <reason>` marker (CLM-7). Identifier-like
   literals (no whitespace, e.g. `"biomarkerKey"`, `"DigitalBiomarker"`, JSON keys) are
   skipped by construction, so the existing self-test fixture `let identifier =
   "biomarkerKey"` still passes; the self-test gains prose-literal cases (positive and
   negative). The 24 current hits are triaged by WP-A.7b: parent-facing copy is reworded;
   official M-CHAT-R/F vocabulary and Debug-only research-classifier strings get
   `claims-ok` with the reason (ADR-0004 d7 allows the instrument's own words; the
   classifier is compiled out of Release, CLM-3). Reasoning on Q5: `CLAUDE.md` forbids
   extending Python with *product logic*; the denylist is a CI lint whose output never
   ships, the change is ~40 lines in one file, and a Rust port would be a new `asd-cli
   claims` subcommand of roughly the same size as the script (metadata limits, Devanagari
   rules, self-test) with a regex-dialect migration (`\b` and case-folding behave the
   same in Python `re` and Rust `regex` for these patterns, but every rule needs a
   re-verified self-test). The port is recorded as backlog WP-D.8 (`I`, about one day)
   to be done the next time the scanner needs a structural change, and `ios_test.sh`
   keeps a single call site so the swap is one line.
5. **Mandatory TOTP is a per-request policy evaluated from database state, not a token
   claim (O-10 defaults: owner, admin, clinician, superadmin; 14 days).**
   `asd-contract::auth_policy` holds `TOTP_REQUIRED_ROLES` and `TOTP_GRACE_DAYS` as
   constants (owner decision; see Open questions) and the enum `TotpEnforcement
   {not_required, grace, required}`. Migration **0023** adds `users.totp_required_by
   timestamptz NULL`. The deadline is set to `now() + grace` the first time a user in a
   required role without TOTP is seen: at login (same transaction as `last_login_at`), at
   `POST /orgs/{id}/users` when the new role is required, and at `PATCH
   /orgs/{id}/users/{uid}` when the role changes into a required one and the column is
   NULL; it is never moved later, and leaving a required role leaves the value in place
   (evaluation keys on the current role, so it is inert). No SQL back-fill: existing users
   start their 14 days at their first login after the deploy, which is also the first
   time they can see the nudge. `auth::totp_policy::evaluate(role, enrolled, required_by,
   now)` is a pure function: `not_required` when the role is not listed or TOTP is
   enrolled; `grace` while `required_by` is NULL or in the future; `required` once it has
   passed.
6. **A user in `required` state can sign in but can only enrol.** Login succeeds as today
   (password, no second factor because none is enrolled) and additionally writes audit
   `auth.login_totp_enrolment_required`. `UserPrincipal::from_request_parts` reads
   `totp_secret_ct IS NOT NULL` and `totp_required_by` with the existing row, evaluates
   the policy, and when it is `required` and the matched route template is not one of
   `GET /v1/users/me`, `POST /v1/users/me/totp/enrol`, `POST /v1/users/me/totp/confirm`,
   `GET /v1/orgs/{id}` (the console shell needs the org name) answers `403` problem
   `totp-enrolment-required` (new slug; detail names the deadline). `POST /v1/auth/logout`
   and `/refresh` carry no bearer and are unaffected. Because the check is per request,
   the first request after `confirm` is unrestricted with the same access token (no
   refresh dance). `DELETE /v1/users/me/totp` stays allowed for required roles (both
   factors are needed to call it); afterwards the user is immediately `required` again
   and can only re-enrol, which is exactly the "replace my authenticator" flow and needs
   no operator. `admin reset-totp` has the same effect. `GET /v1/users/me` gains
   `totp_enforcement` and `totp_required_by`; the console drops its own role list and
   renders from these two fields.
7. **Console: nudge with the date, then a hard redirect to the security page.** In
   `grace` the existing `TotpNudge` adds "by <date>"; in `required` the `[org]/layout.tsx`
   (which already loads data before any page) reads `/users/me` and redirects every path
   except `/{org}/settings/security*` to that page with `?required=1`; the page shows a
   "your role requires two-step sign-in" panel instead of the nudge and hides nothing of
   the existing enrol/confirm UI. `proxy.ts` adds the request header `x-pathname` so the
   layout knows the path (the only change to C.16's file). Any `403
   totp-enrolment-required` reaching a page is handled by the same redirect (the
   `[org]/error.tsx` boundary maps the slug). The mock API models the three states with
   a dedicated mock user so Playwright covers them without a database.
8. **Sequencing honours ADR-0009 D10 for the kit and app, and revises it for the rest.**
   Nothing else in the tree touches Swift, so the Swift 6 packages run first and in
   parallel with the backend policy and the scanner change; the renames run after the
   Swift 6 fixes are integrated (they touch the same files) and alone among iOS packages,
   but in parallel with the console package, which shares no files with them. Wave 1 =
   {A.6a, A.6b, A.6c, B.16, A.7a}; wave 2 = {A.7b, C.17}.
9. **Scope.** In: the seven packages above. Out, with a sentence each in Consequences:
   console idle timeout (F7), org-admin TOTP reset (becomes more useful under D6 and is
   backlog WP-B.17), DEK rotation (F2), worker metrics, HSTS `preload` (WP-E.2),
   `research_consented` (owner, ADR-0009 Q4), fixing the remaining `@preconcurrency`
   warnings in the app.

## Alternatives rejected

- **Passing `-swift-version 6` through `OTHER_SWIFT_FLAGS` / `-Xswiftc`.** SwiftPM warns
  that the mode "may be inconsistent with code generated during the build" and the app's
  flags do not reach the package anyway (trap 1 of the store-readiness packages).
  Rejected.
- **Enabling `SWIFT_TREAT_WARNINGS_AS_ERRORS` for the app together with mode 6.** The
  remaining warnings are in SDK callback closures; silencing them needs `@preconcurrency`
  imports or `MainActor.assumeIsolated`, which hides real problems. The kit keeps the
  gate; the app gets it when the warnings are zero. Rejected for this phase.
- **Renaming the `biomarker` JSON property to `metric` with session 1.9 and
  config-bundle v2.** Three schemas, re-signed baselines, backend dual-read, console
  update and new iOS fixtures for a label already displayed as "Metric". Rejected; the
  rename is about Swift type names the ADR-0004 d7 wording targets.
- **Porting the claims scanner to `asd-cli` before extending it (Q5).** One day of work
  that moves a lint between languages without changing what it checks; it would also
  delay A.7 behind a Rust package. Rejected now; scheduled as WP-D.8 for the next
  structural change.
- **Mandatory TOTP as a restricted JWT (`scope: totp-enrol`).** Needs a token refresh
  after `confirm`, a second code path in the extractor for restricted claims, and makes
  "is this user restricted" depend on token age. The per-request evaluation reuses the
  row the extractor already reads. Rejected.
- **Refusing login outright (`401`) for unenrolled required roles with a separate
  password-only enrolment endpoint.** Duplicates the login path with weaker guarantees
  and no rate-limit sharing. Rejected.
- **Back-filling `totp_required_by` in migration 0023.** Hard-codes the role list and
  grace in SQL next to the Rust constants, and starts the clock for users who have not
  seen the nudge (never logged in since the deploy). Rejected; lazy at login.
- **Blocking `DELETE /v1/users/me/totp` for required roles (`409`).** Would make changing
  authenticators an operator task. Rejected; immediate re-restriction is the simpler
  rule and needs no new endpoint.
- **Making roles and grace environment variables.** A misconfigured deployment could
  silently disable the policy; the values are an owner decision recorded in code and
  the DPIA. Rejected; constants with a unit test that prints them into the schema docs.

## Consequences

- `Package.swift` tools-version 6.0 needs Xcode 16 or newer wherever the kit is built
  (CI `macos-15` has it; `project.yml`'s `xcodeVersion: "15.0"` is an XcodeGen hint and
  is bumped to `"16.0"`). `Package.resolved` does not change.
- Three view files and five test files change name in `git`; `xcodegen generate` picks
  them up (no manual pbxproj edits).
- Migration 0023 (`users.totp_required_by`); the tracker's rule becomes "≥0024".
  `api-v1.json` gains `TotpEnforcement`, two `UserProfile` fields and the slug
  `totp-enrolment-required`; `rust/.sqlx/` is regenerated.
- Every authenticated request reads two more columns from the row the extractor already
  fetches; no extra query.
- The console's role list for the nudge disappears; mock mode gains one more mock user.
- `scripts/ios_test.sh` fails on prose literals from the day WP-A.7b lands; the 24
  current hits are fixed in the same package so the gate is green at integration.
- Not done here, recorded: console idle timeout and absolute session length (O-10 second
  half, F7) is a `proxy.ts`/cookie `max-age` change that should follow the owner's answer
  on session length; org-admin TOTP reset (`POST /v1/orgs/{id}/users/{uid}/totp/reset`,
  owner/admin, audited) is WP-B.17 backlog, worth doing once a pilot org has more than a
  handful of clinicians; DEK rotation, worker metrics, HSTS `preload` unchanged from
  ADR-0009.

## Open questions for the owner

1. **O-10 roles.** Recommendation: `owner`, `admin`, `clinician`, `superadmin` (the roles
   that can read clinical content or change who can). `researcher` and `reviewer` stay
   optional: reviewers see de-identified queues, researchers de-identified exports. Adding
   a role later is one constant and a schema regeneration; removing one is the same.
2. **O-10 grace period.** Recommendation: 14 days from the first login after the
   release, per user (decision 5). Shorter (7 days) is fine for a pilot with few users;
   longer is not recommended because the nudge is visible on every page.
3. **Q5 confirmation.** Recommendation: accept the Python extension (decision 4) and keep
   WP-D.8 in the backlog. Alternative: run WP-D.8 first (about one `implementer` day,
   serial before A.7b).
4. **Console session policy (O-10 second half).** Not designed here. Recommendation: idle
   timeout 30 minutes, absolute 12 hours, implemented as cookie `max-age` plus refresh
   TTL in a later package once the owner confirms the numbers.
