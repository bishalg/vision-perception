# Video Recording Runbook — 5-Minute Devpost Video

> **Deadline**: October 26, 2026 @ 11:59 PM PDT
> **Deliverable**: one video, **≤ 5:00**, uploaded to YouTube (**Public** or **Unlisted**) — link goes on the Devpost form.
> **Everything below was re-verified green on 2026-10-24** (`./scripts/validate_hackathon_readiness.sh` → 8/8 PASS).

Record **shot by shot** (Section 4), assemble in order (Section 5). One take per shot — if you stumble, just re-record that 30–90 s shot, not the whole video.

---

## 1. Pre-flight checklist (15 minutes, do once)

- [ ] Run the full gate: `./scripts/validate_hackathon_readiness.sh` → expect `8/8 PASS`.
- [ ] macOS **Do Not Disturb / Focus ON**; close Slack, Mail, everything else.
- [ ] Terminal: dark theme, **font size 18–20**, window ~1280×720, prompt at top. (Terminal → Settings → Profiles.)
- [ ] Screen recorder: **QuickTime Player → File → New Screen Recording** (or OBS at 1920×1080, 30 fps). Record the **full screen**, mic ON — do a 10 s test and check your voice level.
- [ ] Finder (or Preview) open at `data/flood_nepal_2026/out/` for the image shots.
- [ ] Keep this file + `docs/devpost_submission.md` §2 (storyboard) open on your phone or a second window for the script.

## 2. Artifacts you will show (all exist — do not regenerate)

| Artifact | Path |
|---|---|
| Before/after comparison panel | `data/flood_nepal_2026/out/comparison.png` |
| PRE / POST sub-pixel overlays | `data/flood_nepal_2026/out/pre_overlay.png`, `post_overlay.png` |
| Flood delta summary (the numbers) | `data/flood_nepal_2026/out/flood_delta_summary.md` |
| Signed evidence artifact | `data/flood_nepal_2026/out/flood_delta.json` |
| Architecture diagram (final shot) | `docs/assets/hackathon_architecture.png` |

## 3. One-command re-verify before you hit record

```bash
./scripts/validate_hackathon_readiness.sh && python3 scripts/run_agentic_vision_demo.py --json | grep alertStatus
```
Expect `SUCCESS: All 8/8 Submission Gates Passed!` and `"alertStatus": "DISPATCHED"`.

---

## 4. Shot list (record in any order, assemble in this order)

> Rule check: the official rules require the video to show **the team**, the application working, its architecture, and its principal results — Shot 0 covers the team requirement.

### Shot 0 — Team intro (target 0:00–0:15, ~15 s) — REQUIRED by the rules
**On screen**: you (webcam, or the whole team) — name card overlay optional.

**Say**: *"Hi, we're [name(s)] — [one line each: role/background]. For the OpenCV AI Competition we built vision-perception: sub-pixel perception and autonomous disaster response on OpenCV 5 and AWS."*

### Shot 1 — The hook (target 0:15–0:45, ~30 s)
**On screen**: Preview showing `comparison.png` (before/after panel), then a quick flip between `pre_overlay.png` → `post_overlay.png`.

**Say**: *"In August 2026, a glacier collapse triggered a catastrophic debris flood in Nepal's Bhote Koshi corridor. Early detection saves lives. But standard pixel-difference algorithms fail when clouds and sediment change surface textures. We built vision-perception to solve this."*

### Shot 2 — The C++20 / OpenCV 5 core (target 0:45–1:45, ~60 s)
**On screen**: terminal. Paste and run:

```bash
./scripts/run_flood_pipeline.sh
```
(≈10 s of stage output — let STAGE 6 finish.) Then, in a second pane or after a cut:

```bash
ctest --test-dir build --output-on-failure | tail -3
```
(Expect `100% tests passed, 41 tests`. This runs ~1 min — record only the tail, or run it before recording and paste the last lines.)

Then open `data/flood_nepal_2026/out/flood_delta_summary.md` in the editor and scroll the results table.

**Say**: *"Under the hood is a production C++20 and OpenCV 5 monorepo using Hessian tensors and Taylor zero-crossing to extract sub-pixel curvilinear ridges — 41 test suites green, sub-pixel accuracy of three-hundredths of a pixel. We proved our dual controls: plus 121 percent channel widening, with spatial control at 0.151x and temporal baseline at 0.905x."*

### Shot 3 — Agentic Vision: the 3-node trace (target 1:45–3:15, ~90 s)
**On screen**: terminal, maximized.

```bash
python3 scripts/run_agentic_vision_demo.py
```
Let the banner print, then walk through: Node 1 `getFloodEvidence` → evidence token, Node 2 policy thresholds, Node 3 `dispatchFloodAlert` → DynamoDB idempotency + S3 receipt. If it asks for input, accept the defaults (this is the interactive demo). ~90 s of narration fits naturally while it prints.

**Say**: *"We targeted the Agentic Vision award by building a true Perception-to-Action loop. When flood_delta.json reaches S3, it wakes our Bedrock Agent. The agent calls getFloodEvidence, evaluates our strict thresholds, acquires a cryptographic evidence token, and calls dispatchFloodAlert. DynamoDB validates the token and prevents duplicate alerts."*

**Optional +20 s (strong Agentic evidence — the rules ask for vision output *changing* the action)**:

```bash
python3 scripts/run_agentic_vision_demo.py --scenario marginal
```
Point at `BELOW POLICY THRESHOLDS — STANDBY` and `/dispatchFloodAlert — NOT CALLED`.

**Say**: *"And the evidence gates the action in both directions — feed the same pipeline pre-disaster magnitudes and the agent records a standby decision without dispatching."*

### Shot 4 — Best Use of COOL: the benchmark (target 3:15–4:15, ~60 s)
**On screen**: terminal.

```bash
./scripts/benchmark_cool_vs_x86.sh --skip-build
```
The SIMD inspection block prints first (NEON_DOTPROD / NEON_FP16 — hold on it ~5 s), then 100 iterations run (~30 s), then the result box prints. Cut out the middle of the run; keep the SIMD block and the result box.

Then briefly show `docs/aws_benchmark_guide.md` §2 (the EC2 Graviton3 commands) to land the AWS story.

**Say**: *"Scale-space convolutions are computationally heavy. Our measured 100-iteration run sustains a 213.6 millisecond P50 latency — over fifty thousand disaster scenes per dollar — with Arm NEON SIMD verified active. The identical harness runs on AWS Graviton3 with the Cloud-Optimized OpenCV Library using our one-command EC2 reproduction guide."*

### Shot 5 — Architecture & close (target 4:15–5:00, ~45 s)
**On screen**: Preview, full screen: `docs/assets/hackathon_architecture.png`. Slow zoom/pan left → right if your tool supports it (QuickTime doesn't — just hold and point with the cursor).

**Say**: *"From sub-pixel C++ tensor extraction to autonomous, evidence-gated AWS Bedrock agents, vision-perception proves that rigorous computer vision and modern agentic AI can save lives. The full build, tests, and judges' quickstart are in the repository. Thank you."*

---

## 5. Assembly & upload (30 minutes)

1. Assemble shots in order in iMovie / CapCut / DaVinci Resolve (any is fine). Add:
   - Title card at 0:00 (3 s): project name + tagline *"Powered by OpenCV 5, AWS Bedrock Agents, and COOL on Graviton3"*.
   - Name cards between shots 2/3 and 3/4 ("Agentic Vision", "Use of COOL").
2. Trim to **≤ 5:00**. Export 1920×1080.
3. Upload to YouTube as **Unlisted** (judge-accessible) — title: `vision-perception — OpenCV AI Competition 2026`. Add a 2-line description with the repo link.
4. Copy the YouTube URL into the Devpost form.
5. Devpost form checklist: paste §1 text from `docs/devpost_submission.md` · upload `docs/assets/devpost_thumbnail.png` · upload `docs/assets/hackathon_architecture.png` as an image · paste video URL · repo access for judges (private repo → add judges / make judge-accessible per Devpost instructions).

## 6. Fallbacks (if something breaks mid-recording)

| Problem | Fix |
|---|---|
| Demo script fails | Re-run `./scripts/validate_hackathon_readiness.sh` — it pinpoints the failed gate; every gate passed on 2026-10-24, so a failure means local state changed. |
| Terminal text too small on video | Re-record only that shot after bumping font to 20. |
| Mic pops / noise | Record voiceover separately in iMovie over screen footage — storyboard text is the script. |
| Over 5:00 after assembly | Cut Shot 2's ctest tail and Shot 4's mid-run first. |
