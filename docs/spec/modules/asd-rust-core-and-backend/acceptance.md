# ASD Rust Core and Backend - Acceptance

Scenarios an owner or reviewer can run. Each cites the spec ids it proves.

| # | Scenario | Proves |
| --- | --- | --- |
| 1 | `scripts/rust_test.sh` passes on a clean checkout with Docker running (Postgres service). | WS-1..5, GATE-5 |
| 2 | `cargo run -p asd-cli -- schema export --out /private/tmp/schemas` then `diff -r` against `docs/spec/schemas/asd/` is empty; editing one field in `SessionIngestRequest` makes `schema check` fail. | CON-3 |
| 3 | `asd-cli schema validate docs/spec/schemas/asd/session-v1.6.json apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json` passes; the same with `sessionId` removed fails. | CON-4 |
| 4 | `cargo test -p asd-core` passes every golden vector; `swift test --scratch-path /private/tmp/asd-kit --filter GoldenParityTests` passes on the same files. | CORE-1..7, FFI-4 |
| 5 | In the iOS simulator a stimulus session produces the same `PerVideoSessionScore` JSON and `AttentionFlag` as the build before WP-A.11 (compare the vault export). | FFI-3 |
| 6 | `docker compose -f rust/docker-compose.yml up`; `POST /v1/orgs/{id}/devices/enrol-code` from the console login; register a device with the code; the same code again without `device_id` → 410; with `device_id` → 200 `status:"rotated"`. | CON-10, CON-11, AUTH-3 |
| 7 | Post the iOS golden fixture to `POST /v1/sessions` with the device bearer: 201 with `frames_upload_url`; repeat → 200 `duplicate:true` identical body; repeat with a changed `durationSec` → 409. 20 parallel identical posts → one row in `sessions`. | ING-1, CON-17 |
| 8 | Post with `captureSource:"simulated"` → 400; `attentionFlag:null` with `validRatio 0.5` → stored `unreliable`; with valid per-video rows → the flag `asd-core` computes. | CON-13, CON-15 |
| 9 | Console in `CURV_API_MODE=live`: login, sessions table, detail Clips tab shows per-metric rows with expected/recorded/deviation; Report tab shows the device-uploaded PDF. | API-2, API-3, CON-19 |
| 10 | Refresh with a rotated-out refresh token → 401 and every token of that family is revoked (next refresh with the newest token also fails). | AUTH-2 |
| 11 | A `viewer` calling `PATCH /orgs/{id}` → 403; any user calling `GET /sessions/{id}` of another org → 404; the `audit_log` has one row per mutation and the app DB role cannot `UPDATE` it. | AUTH-4, AUTH-5 |
| 12 | Publish catalog + baselines + scoring profile, `POST /config/bundles`; `GET /config/bundle` with the device bearer returns a bundle whose signature `asd-ffi verify_bundle` accepts; flip one byte → rejected; `If-None-Match` with the ETag → 304. | CFG-1, CON-6 |
| 13 | `asd-cli synth --seed 7 --count 10` twice gives identical files; every file validates against `session-v1.6.json`; `asd-cli norms aggregate` over a directory of 30 synthetic vault exports withholds every cell with n < 12 and emits `status:"candidate-collected"`. | CORE-8, CORE-9, GATE-2, GATE-3 |
| 14 | `grep -r "participant_token\|@" <api log>` over the run of scenarios 6–9 finds no participant token or e-mail. | OBS-1 |
| 15 | `cargo deny check` fails when a GPL crate is added to `asd-core`. | WS-3 |
| 16 | After GATE-4 `git ls-files | grep -E "asd_synth|asd_norm_aggregator|check_session_schema_parity|asd_research_server"` is empty and `ios.yml` is green. | GATE-1..4 |
