# Master 7-Week Daily Execution Plan
## Dual Hackathon Track: OpenCV AI 2026 (AWS) & Nebius x NVIDIA Global AI Hackathon

> **Current Date**: September 12, 2026  
> **OpenCV AI Competition (AWS) Deadline**: October 27, 2026 (45 Days)  
> **Nebius x NVIDIA Hackathon Deadline**: October 30, 2026 (48 Days)  
> **Confidentiality Guardrail**: Core C++ mathematical engine (`core/`, `adapters/`, `domains/`) stays closed-source & proprietary. All public hackathon code is isolated to external client agents.

---

```
                       7-WEEK HIGH-LEVEL ROADMAP
  ┌──────────────────────────────────────────────────────────────────────┐
  │ W1 (Sep 12-18): Core Engine Isolation & Standardized Telemetry Schema│
  │ W2 (Sep 19-25): AWS Graviton3 & COOL Benchmark Execution (AWS Grant) │
  │ W3 (Sep 26-Oct 2): Nebius Token Factory & NVIDIA Nemotron 3 Adapter  │
  │ W4 (Oct 03-09): Physical AI Stream Pipeline (CCTV & Video Feeds)     │
  │ W5 (Oct 10-16): End-to-End Integration, Failover & UI Twin Polish    │
  │ W6 (Oct 17-23): OpenCV AI Submission Sprint (Doc, Video, Submit)     │
  │ W7 (Oct 24-30): Nebius x NVIDIA Submission Sprint (Repo, Video, Sub) │
  └──────────────────────────────────────────────────────────────────────┘
```

---

## Week 1: Core Engine Telemetry Export & Clean IP Boundary
*Focus: Stabilize the core C++ perception output so both cloud systems can consume it without ever touching closed-source code.*

- **Day 1 (Sat, Sep 12): Registration & Baseline Audit**
  - Register for Nebius x NVIDIA Hackathon on Devpost.
  - Review AWS account credits / grant status for OpenCV AI Competition.
  - Verify all C++ unit tests pass (`ctest --output-on-failure`, 20/20 green).
- **Day 2 (Sun, Sep 13): Telemetry Schema Design (`EvidenceExportSchema`)**
  - Define language-neutral `flood_evidence_v1.json` schema (bounding polygons, centerline polylines, expansion ratio, dual-control scores, timestamp, SHA256 integrity hash).
  - Ensure zero C++ internal pointer or class leaks in exported JSON.
- **Day 3 (Mon, Sep 14): Core CLI Exporter Integration**
  - Add `--export-evidence <path>` flag to `curv_cli --domain geospatial`.
  - Validate output format on pre/post Sentinel-2 test scenes.
- **Day 4 (Tue, Sep 15): Dual-Control Assertion Hardening**
  - Ensure `flood_delta.json` explicitly flags control status:
    - `spatial_control_pass: bool` (slope > 20° baseline)
    - `temporal_control_pass: bool` (historical seasonal baseline)
    - `alert_recommendation: "CRITICAL" | "WARNING" | "NORMAL"`
- **Day 5 (Wed, Sep 16): Mock Telemetry Test Harness**
  - Create test fixture generator `tools/generate_mock_flood_stream.py` emitting synthetic time-series telemetry for cloud agent testing.
- **Day 6 (Thu, Sep 17): S3 & Local File Mirror Adapter**
  - Build lightweight sync script that pushes generated `flood_delta.json` + overlay images to S3 / local web server for consumption.
- **Day 7 (Fri, Sep 18): Week 1 Review & Gate Check**
  - Verify C++ build and CI tests remain 100% green.
  - Verify JSON schema documentation is complete in `docs/evidence_schema.md`.

---

## Week 2: AWS Graviton3 & COOL Benchmark Execution (OpenCV AI Path)
*Focus: Fulfill the Best Use of COOL Award ($1,000) and prepare for the mandatory AWS grant check-in.*

- **Day 8 (Sat, Sep 19): AWS Graviton3 (c7g) Environment Provisioning**
  - Launch an AWS Graviton3 EC2 instance (`c7g.xlarge`, Ubuntu 24.04 ARM64).
  - Clone benchmark harness dependencies onto the instance.
- **Day 9 (Sun, Sep 20): COOL (Cloud-Optimized OpenCV Library) Installation**
  - Subscribe and install COOL via AWS Marketplace / pip on the Graviton3 instance.
  - Verify SIMD vectorization with `python3 -c "import cv2; print(cv2.getBuildInformation())"`.
- **Day 10 (Mon, Sep 21): Benchmark Execution: Steger Hessian vs. Standard OpenCV**
  - Run `scripts/benchmark_cool_vs_x86.sh` across 100 iterations.
  - Profile P50, P90, P95 latencies and CPU utilization for Gaussian derivatives and Hessian decomposition.
- **Day 11 (Tue, Sep 22): Cost & Latency Modeling**
  - Compare Graviton3 ($0.148/hr) vs. standard x86 ($0.170/hr) per 1,000 square km river scene.
  - Export structured benchmark results to `docs/aws_benchmark_results.json`.
- **Day 12 (Wed, Sep 23): Complete AWS 30-Min Zoom Check-In Prep**
  - Prepare 3-slide progress summary for the OpenCV organizing committee (required for 2nd 50% grant installment).
  - Review architecture diagram in `aws_infra/README.md`.
- **Day 13 (Thu, Sep 24): CDK IaC Deployment Dry-Run**
  - Run `cdk synth` and `cdk diff` in `aws_infra/`.
  - Validate `StorageStack`, `AlertStack`, and `AgentStack` CloudFormation outputs.
- **Day 14 (Fri, Sep 25): Week 2 Review & Benchmark Guide Finalization**
  - Commit final benchmark tables to `docs/aws_benchmark_guide.md`.
  - Sign-off on AWS COOL award requirements.

---

## Week 3: Nebius Token Factory & NVIDIA Nemotron 3 Adapter
*Focus: Build the open-source client agent for the Nebius x NVIDIA Hackathon.*

- **Day 15 (Sat, Sep 26): Nebius Token Factory Onboarding**
  - Access Nebius Token Factory Console & generate API keys.
  - Test inference with `nvidia/nemotron-3-8b` and `nemotron-3-super` using the OpenAI-compatible REST API.
- **Day 16 (Sun, Sep 27): Standalone Open-Source Repo Setup**
  - Create external repository `river-sentinel-nemotron`.
  - Add Apache 2.0 `LICENSE`, README template, and directory structure (`src/`, `tests/`, `examples/`).
- **Day 17 (Mon, Sep 28): Nemotron Reasoning Engine Prompt Design**
  - Implement `NemotronFloodEvaluator` in Python:
    - Ingests `flood_evidence_v1.json`.
    - Enforces step-by-step reasoning: evaluates channel width surge %, verifies spatial & temporal control gates, checks confidence bounds.
- **Day 18 (Tue, Sep 29): Structured Tool-Use & Civil Defense Action Dispatch**
  - Define tool definitions: `issue_civil_defense_alert(zone, severity, evacuation_radius)` and `request_secondary_sensor_verification(cctv_id)`.
  - Implement structured JSON output formatting from Nemotron.
- **Day 19 (Wed, Sep 30): Fallback & Model Tiering Strategy**
  - Configure Nemotron 3 Nano for fast everyday sanity polling; promote to Nemotron 3 Ultra when anomalous flood delta is detected.
- **Day 20 (Thu, Oct 01): Unit & Integration Testing for Nebius Agent**
  - Write pytest test suite verifying Nemotron output against 5 synthetic disaster scenarios (flash flood, false cloud reflection, dry riverbed baseline).
- **Day 21 (Fri, Oct 02): Week 3 Review & Open Repo Check**
  - Ensure zero proprietary C++ code or headers exist in the `river-sentinel-nemotron` repo.
  - Tag initial working release `v0.1.0`.

---

## Week 4: Physical AI Stream Pipeline (River CCTV & Sensor Adapters)
*Focus: Fulfill Physical AI track requirements by connecting edge camera streams and time-series telemetry.*

- **Day 22 (Sat, Oct 03): CCTV Stream Frame Ingestion Harness**
  - Test `adapters/cctv/CctvStreamAdapter.cpp` with RTSP / local MP4 river camera loop.
  - Validate frame extraction stability over 1-hour sustained run.
- **Day 23 (Sun, Oct 04): Water Level Edge Metric Extractor**
  - Implement optical waterline gauge tracking module (detecting riverbank waterline boundary drift in CCTV frames).
- **Day 24 (Mon, Oct 05): Edge-to-Cloud Telemetry Bridge**
  - Connect CCTV waterline metric output into the `flood_evidence_v1.json` stream.
  - Enable multi-sensor cross-correlation: Satellite flood extent + Terrestrial CCTV waterline rise.
- **Day 25 (Tue, Oct 06): Nebius Serverless Job Dispatch**
  - Package asynchronous sensor batch processing as a Nebius Serverless Job.
  - Test invocation triggered by new imagery arrival.
- **Day 26 (Wed, Oct 07): Physical AI Hardware Demonstration Setup**
  - Prepare hardware/edge setup (Raspberry Pi, Jetson, or live camera feed display) showing physical frame ingestion in real-time.
- **Day 27 (Thu, Oct 08): Dual Control Cross-Sensor Verification**
  - Test scenario where satellite detects water expansion AND river CCTV confirms rapid cresting $\to$ Nemotron issues level-4 immediate evacuation.
- **Day 28 (Fri, Oct 09): Week 4 Review & Physical AI Checkpoint**
  - Confirm compliance with Physical AI track rule: live physical camera or key application modules demonstrating sensing $\to$ reasoning $\to$ action.

---

## Week 5: End-to-End System Validation & Studio Twin Polish
*Focus: Connect the full feedback loop and polish the user-facing demonstration.*

- **Day 29 (Sat, Oct 10): CurvStudio / Digital Twin HUD Polish**
  - Verify web UI (`apps/annotation_studio/`) displays flood vector polygons, channel width telemetry, and live camera feed side-by-side.
- **Day 30 (Sun, Oct 11): Bedrock Agentic Loop Verification (OpenCV Path)**
  - Test full AWS loop: `flood_delta.json` upload to S3 $\to$ Lambda trigger $\to$ Bedrock `getFloodEvidence` $\to$ Bedrock `dispatchFloodAlert` $\to$ SNS email delivery.
- **Day 31 (Mon, Oct 12): Nebius Nemotron Loop Verification (Nebius Path)**
  - Test full Nebius loop: CLI export $\to$ Nemotron 3 reasoning $\to$ Webhook / Civil defense alert dispatch $\to$ Terminal trace logging.
- **Day 32 (Tue, Oct 13): Failure Handling & Idempotency Testing**
  - Test corrupted frame rejection, cloudy scene rejection, and duplicate hash suppression via DynamoDB.
- **Day 33 (Wed, Oct 14): Performance & Latency Audit**
  - Measure end-to-end latency from raw frame arrival to final dispatch on both platforms.
- **Day 34 (Thu, Oct 15): Live Demo Endpoint Hosting**
  - Deploy a public or judge-accessible web demo endpoint (or tunnel via Cloudflare/ngrok) hosting the studio visualization.
- **Day 35 (Fri, Oct 16): Dual Track Milestone Sign-Off**
  - Both technical backends frozen. Transition to documentation, videos, and Devpost packaging.

---

## Week 6: OpenCV AI Competition 2026 Submission Sprint
*Deadline: October 27, 2026 @ 12:15 PM GMT+5:30*

- **Day 36 (Sat, Oct 17): Technical Report Drafting**
  - Draft comprehensive report covering: Problem Statement, OpenCV 5 usage, COOL Graviton benchmarks, AWS Bedrock agentic loop, ethical/responsible deployment.
- **Day 37 (Sun, Oct 18): Architecture Diagrams & Visual Assets**
  - Generate high-res diagrams of the multi-stage pipeline, AWS CDK cloud topology, and Agentic Decision Tree.
- **Day 38 (Mon, Oct 19): Video Scripting & Storyboard ($\le$ 5 minutes)**
  - Outline 5-minute video: Problem (1 min), OpenCV 5 + COOL Tech (1.5 min), AWS Bedrock Loop in Action (1.5 min), Real-world Impact & Results (1 min).
- **Day 39 (Tue, Oct 20): Screen Recording & Demo Capture**
  - Record terminal CLI runs, Graviton benchmark console, AWS Bedrock agent traces, and CurvStudio HUD.
- **Day 40 (Wed, Oct 21): Video Editing & Voiceover**
  - Assemble video with audio narration, subtitles, and callouts highlighting OpenCV 5 and COOL.
- **Day 41 (Thu, Oct 22): Judge-Accessible Repository Packaging**
  - Ensure pinned dependencies, clear build instructions, and README are complete in the private submission repository.
- **Day 42 (Fri, Oct 23): Final Review & Dry-Run Submission**
  - Review against all 6 judging criteria (Technical 30%, Innovation 20%, Impact 20%, UX 10%, Doc 10%, Cloud 10%).
- **Day 43–45 (Sat–Mon, Oct 24–26): SUBMIT OPENCV AI 2026**
  - Submit on Devpost at least 24 hours ahead of the Oct 27 deadline.

---

## Week 7: Nebius x NVIDIA Global AI Hackathon Submission Sprint
*Deadline: October 30, 2026 @ 10:30 PM GMT+5:30*

- **Day 44 (Sun, Oct 25): Open-Source Repo Polish (`river-sentinel-nemotron`)**
  - Ensure Apache 2.0 license is visible, README has quickstart guide (`pip install`, `python -m river_sentinel`), and sample datasets are included.
- **Day 45 (Mon, Oct 26): Written Project Description & Pre-existing Work Disclosure**
  - Write detailed Devpost description explaining: Physical AI architecture, Nebius Token Factory integration, and clear disclosure of what was developed during the hackathon.
- **Day 46 (Tue, Oct 27): Video Scripting & Recording ($\le$ 3 minutes)**
  - Strictly $\le$ 3 minutes. Must include:
    - $\ge$ 1 minute showing physical application modules / camera feeds in action.
    - Clear explanation of Nebius Token Factory and NVIDIA Nemotron usage.
- **Day 47 (Wed, Oct 28): Video Editing & Feedback Preparation**
  - Finalize YouTube video upload.
  - Draft constructive feedback for Nebius Token Factory & NVIDIA Nemotron (qualifies for the $100 Most Valuable Feedback award).
- **Day 48 (Thu, Oct 29): Submission Dry-Run & Link Testing**
  - Verify public GitHub repo clone works in a clean environment.
  - Verify YouTube video audio and accessibility.
- **Day 49 (Fri, Oct 30): SUBMIT NEBIUS x NVIDIA HACKATHON**
  - Submit on Devpost comfortably before the 10:30 PM GMT+5:30 deadline.
