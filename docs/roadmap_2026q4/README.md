# vision-perception — Q4 2026 → Q1 2027 Handover & Roadmap

> **Written:** 2026-10-27 · **Author:** architecture pass (Claude Fable 5.1, scans by Sonnet/Haiku agents) · **Owner:** Bishal Ghimire
> **Status of repo at writing:** branch `dev`, HEAD `71c657e`, public on GitHub, OpenCV AI Competition 2026 submitted (closed 2026-10-27).

This folder is the single entry point for any human or AI agent picking up this monorepo after the hackathon. It replaces scattered sprint trackers as the **planning source of truth** for the next ~4 months. Older docs (`docs/asd_app_sprint_tracker.md`, `docs/flood_robust_roadmap.md`, `docs/palm_*_roadmap.md`) remain as history; where they disagree with this folder, this folder wins.

## Priority order (decided 2026-10-27)

| Rank | Domain | Why | Doc |
|---|---|---|---|
| **1** | **Autism / ASD gaze app + backend + admin panel** | Demo in 2–3 weeks; needs a real backend, data organisation, and configuration ("training-level") controls. Most product value. | `01`, `02`, `03`, `04` |
| 2 | Repo hygiene for a now-public repo | Personal data, placeholder IRB text and a demo login are in the tree. Cheap to fix, embarrassing to leave. | `00` |
| 3 | Flood / CurvFlood | Hackathon is submitted; maintain, fix known defects, decide on the Nebius track (deadline **2026-10-30**). | `05` |
| 4 | Core platform / CI / licensing | Keeps everything else honest; small, continuous. | `07` |
| 5 | Palm hand-scan + Vastu | Explicitly deprioritised. Freeze, hygiene only. | `06` |

## Files

| File | What it is | Read when |
|---|---|---|
| `00_repo_state_and_urgent_actions.md` | What the three scans found; the 10 things to do **this week** | First, always |
| `01_asd_product_and_demo_plan.md` | What the ASD app really is today vs what the pitch claims; the 3-week demo plan | Before touching the iOS app |
| `02_asd_backend_architecture.md` | Backend design: stack, data model, API, auth, deployment, ADRs | Before writing any backend code |
| `03_asd_admin_panel_spec.md` | Admin panel feature spec: data organisation, stimulus/baseline/scoring configuration, review queue, cohorts, model registry | Before writing any admin UI |
| `04_asd_phased_work_packages.md` | Phase-by-phase work packages (WP-A.x … WP-E.x) with acceptance criteria and suggested agent per package | When starting a work package |
| `05_flood_maintenance_roadmap.md` | Post-hackathon flood/AWS fixes, data reproducibility, Nebius decision | When working on flood |
| `06_palm_maintenance_roadmap.md` | Freeze plan for palm and vastu apps | When working on palm |
| `07_core_platform_hygiene.md` | Core invariant enforcement, CI, licensing, build hygiene | When touching `core/`, CI, or licences |

## How an AI agent should use this folder

1. Read `00` fully. Read the domain doc for your task. Read the WP entry in `04` you were assigned.
2. Each WP lists: goal, files it may touch, files it must not touch, acceptance criteria, tests to add, and the model tier suggested (`architect` = Fable for design only; `senior-implementer` = Opus; `implementer` = Sonnet). Follow the repo-owner's routing rule in `~/.claude/CLAUDE.md`: design with Fable, build with Opus/Sonnet.
3. Never commit: child telemetry, model weights without a manifest + licence file in `models/`, personal photos, credentials, or placeholder institution/IRB names.
4. Run the relevant gate before reporting done: `scripts/ios_test.sh` (ASD iOS), `pytest tests/ aws_infra/tests tools/` (Python), `cargo test --manifest-path rust/Cargo.toml` and `cargo run -p asd-cli -- schema check` (Rust ASD core and contract), `cmake --preset release && ctest --preset release` (C++), `python3 scripts/check_asd_claims.py --swift --secrets` (claims).
5. Update the **status table at the top of `04`** when a WP changes state. Do not create a new tracker file.
6. Report back with: what changed, test output, what was skipped and why.

## Conventions used in these docs

- Dates are absolute (YYYY-MM-DD). "Now" = 2026-10-27.
- "Must / should / may" are RFC-2119 style.
- `H-n` = a human decision only the owner can make. They are collected in `00 §4`.
- Work-package IDs: `WP-A.n` ASD iOS app, `WP-B.n` backend, `WP-C.n` admin panel (console), `WP-D.n` data/ML, `WP-E.n` compliance/release, `WP-F.n` flood, `WP-P.n` palm, `WP-X.n` platform.
