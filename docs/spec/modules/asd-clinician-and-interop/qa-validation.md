# ASD Clinician Workspace and Interoperability Bridge - QA Validation

## Layers

1. `asd-export` unit and golden tests: pure, no network, no database. CSV and HL7
   goldens are committed text files. XLSX is read back with `calamine`. PDF is checked by
   structure and text extraction. No binary goldens are committed (`*.pdf` and `*.zip` are
   git-ignored).
2. Backend integration tests (`rust/crates/asd-backend/tests/`): `sqlx::test` schema per
   test, in-memory blob store, a fixed test `ASD_DATA_KEK`, an in-process `russh` SFTP server
   and a local HTTPS test receiver (with the netguard allow-list switched on for the test
   only).
3. Cross-implementation parity: FHIR ids and the gaze Observation against
   `apps/ios_asd_gaze_app/Tests/Fixtures/expected-bundle.json`. Console zod schemas against
   `docs/spec/schemas/asd/api-v1.json` (`contract.test.ts`).
4. Static gates: `scripts/rust_test.sh` (incl. `cargo deny` for `rust_xlsxwriter`, the PDF
   engine, `reqwest`, `russh*`, `chrono-tz`), console lint, typecheck, test and build.
5. Optional external validators, run by hand or in a non-gating CI job: veraPDF
   (PDF/A-2b), HL7 FHIR validator jar (R4), HAPI HL7 v2 TestPanel or Mirth Connect (ORU^R01).

## Commands

```bash
cd ~/Documents/WORK/Code/vision-perception
scripts/rust_test.sh
CARGO_TARGET_DIR=/private/tmp/asd-rust cargo test --manifest-path rust/Cargo.toml -p asd-export
CARGO_TARGET_DIR=/private/tmp/asd-rust cargo check --manifest-path rust/Cargo.toml -p asd-export \
  --no-default-features --target aarch64-apple-ios
DATABASE_URL=postgres://asd:asd@localhost:5432/asd_test ASD_DATA_KEK=$(openssl rand -base64 32) \
  cargo test --manifest-path rust/Cargo.toml -p asd-backend
cargo run --manifest-path rust/Cargo.toml -p asd-cli -- schema check
cd apps/curv_console && npm run lint && npm run typecheck && npm test && npm run build
```

## Spec id → proof

| Ids | Test or command |
| --- | --- |
| RBAC-1..5 | `asd-backend/tests/rbac_clinical.rs`; `auth/rbac.rs` unit tests |
| KEY-1..4, KEY-6 | `asd-backend/tests/crypto.rs`; `log_redaction.rs` extended with clinical field names |
| KEY-5 | Operator runbook check in WP-E.6 (not automated) |
| KEY-7 | `asd-backend/tests/review_note_encryption.rs` |
| MRN-1..6 | `asd-backend/tests/patient_refs.rs`; `asd-export/tests/deid.rs` (MRN never survives) |
| CW-1..3 | `asd-backend/tests/{worklist,timeline}.rs` |
| SIGN-1..10 | `asd-backend/tests/{clinical_review,signing}.rs`; `migrations_0008.rs` (triggers); `asd-contract/tests/clinical_hash.rs` |
| NOTE-1..7 | `asd-backend/tests/{notes,letters,clinical_summary}.rs`; `routes/users.rs` tests for profile fields |
| XC-1..8 | `asd-export/tests/{deid,policy,codes,model_roundtrip,watermark}.rs`; iOS-target `cargo check` |
| CSV-1..4, XLSX-1..3 | `asd-export/tests/{csv,xlsx}.rs` |
| PDF-1..6 | `asd-export/tests/pdf.rs`; veraPDF (optional) |
| FHX-1..8 | `asd-export/tests/{fhir_clinical,fhir_parity}.rs`; FHIR validator (optional) |
| HL7-1..7 | `asd-export/tests/hl7v2.rs`; HAPI TestPanel (optional) |
| EXJ-1..9 | `asd-backend/tests/{exports,export_jobs,runner_run_after,blobs_get}.rs` |
| INTG-1..5, DLV-1..6 | `asd-backend/tests/{integrations,webhook,sftp,netguard,auto_delivery}.rs` |
| DLV-7 | WP-I.9/I.10 tests against a HAPI FHIR container (later) |
| MIG-1..4 | `asd-backend/tests/migrations.rs` (applies all migrations from empty; checks version list has no 0007 added after 0008) |
| CTR-1..3 | `asd-cli schema check`; `apps/curv_console/src/lib/api/contract.test.ts` |
| UI-1..5 | console vitest suites listed per package; acceptance scenarios 1, 6, 8, 21 |

## Manual review checklist (per release that touches this module)

- Every user-visible string added in `asd-export/src/text.rs` passes the claims denylist
  test, and the mandatory sentences appear in PDF, XLSX README, HL7 NTE and FHIR narrative.
- No new endpoint takes an MRN, note text or impression in its path or query.
- No new table stores free text without `_ct` and `_key_id` columns.
- No new job handler performs network I/O inside a DB transaction.
