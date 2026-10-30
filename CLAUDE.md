# Project rules: vision-perception

## Rust is the core language (owner decision, 2026-10-30)

All core, calculation and math logic is written in Rust and shared across every platform. Each OS app does only its own high-level work in its native language.

- **Rust (shared, one implementation):** scoring, metrics, gaze/AOI math, norms and baselines, validation, schema and contract types, signing/verification, sync protocol logic, and the backend service (Axum + sqlx + Postgres).
- **Native per OS (thin):** UI, camera/ARKit/sensor capture, OS permissions, Keychain/Keystore, notifications, platform glue. Swift for iOS/visionOS, Kotlin for Android, TypeScript for the web console. These call into the Rust core (UniFFI or C ABI for apps, WASM for web) and never re-implement its logic.
- **No new Python.** Do not add Python services, backends or core logic. The Python ASD backend was removed. Existing Python scripts and tools stay only as dev/CI tooling and are not extended with product logic; replace them with Rust when touched.
- **One source of truth for contracts:** API and session schemas are defined once in Rust types and exported as JSON Schema (e.g. `schemars`). Native clients and the console are checked against that schema. Until the Rust crate exists, `docs/spec/schemas/asd/session-v1.6.json` is the contract.
- **Existing C++ core** (`core/`, palm and flood): legacy, maintained as is. Do not rewrite it unless the owner asks. New shared logic goes in Rust.
- If a task would put core/math logic in Swift, Kotlin, TypeScript or Python, stop and put it in Rust instead, or ask.

## Planning
Roadmap and work-package tracker: `docs/roadmap_2026q4/` (update the status table in `04_asd_phased_work_packages.md`; do not create new tracker files).
