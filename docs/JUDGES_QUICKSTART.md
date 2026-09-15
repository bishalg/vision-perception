# OpenCV AI Competition 2026 — Judges & Reviewers Quickstart Guide

> **Project Title**: vision-perception: Sub-Pixel Curvilinear Perception & Autonomous Disaster Response  
> **Tagline**: *Powered by OpenCV 5, AWS Bedrock Agents, and COOL on Graviton3*  
> **Target Tracks & Special Awards**:  
> 1. **Real-World Impact Track** (Langtang Himal / Bhote Koshi Glacier-Collapse Disaster Early Warning)  
> 2. **Best Use of COOL Award ($1,000)** (Cloud-Optimized OpenCV Library on AWS Graviton3)  
> 3. **Agentic Vision Award ($1,000)** (Authentic Perception $\to$ Reasoning $\to$ Action loop on AWS Bedrock)  

---

## ⚡ 60-Second Evaluation Quickstart (Zero Cloud Dependencies Required)

Judges can verify the entire multi-stage system locally on any macOS or Linux machine without needing an active AWS account or pre-configured cloud credentials.

### 1. Run Complete Submission Readiness Audit (All 8 Gates)
```bash
./scripts/validate_hackathon_readiness.sh
```
*Executes all 41 C++ CTests, 11 AWS Agent tests, 44 Tool tests, full E2E pipeline integration, format gates, and verifies all submission deliverables.*

### 2. Run Interactive Agentic Vision Loop Demonstration
```bash
python3 scripts/run_agentic_vision_demo.py
```
*Walks through the authentic 3-node Bedrock Agent trace: autonomous perception via `/getFloodEvidence`, multi-condition disaster policy verification, and action execution via `/dispatchFloodAlert` with DynamoDB idempotency enforcement and immutable S3 audit receipt generation.*

### 3. Run C++20 Curvilinear Flood Perception Pipeline
```bash
./scripts/run_flood_pipeline.sh
```
*Runs the C++20 Steger line extractor on pre-event (2026-08-24) and post-event (2026-08-27) Sentinel-2 imagery. Extracts sub-pixel ridges with `< 0.0301 px` analytical RMSE, generating vector SVGs and visual overlays in under 100ms.*

### 4. Run Quantitative Flood Delta & Cryptographic Signing
```bash
python3 scripts/compute_flood_delta.py
```
*Calculates sub-pixel channel widening (+121.1%), flood signature surge (+88.4%), and validates off-river spatial slope controls (0.151x, PASS) and temporal baseline controls (0.905x, PASS). Generates cryptographically signed `data/flood_nepal_2026/out/flood_delta.json`.*

### 5. Inspect Cloud-Optimized OpenCV (COOL) Benchmark Suite
```bash
./scripts/benchmark_cool_vs_x86.sh --dry-run --skip-build
```
*Demonstrates the SIMD vector inspection (`cv2.getBuildInformation()`), latency profiling harness, and cost-per-scene modeling targeting AWS Graviton3.*

---

## 🏗️ Architecture: Perception $\to$ Decision $\to$ Action

```text
  ┌───────────────────────────┐      ┌──────────────────────────┐      ┌─────────────────────────────┐
  │ 1. S3 INGESTION GATE      │      │ 2. AGENT PERCEPTION TOOL │      │ 3. AUTONOMOUS REASONING     │
  │ • Sentinel-2 delta upload │ ───► │ • Bedrock Agent calls    │ ───► │ • Evaluates flood surge     │
  │ • S3:ObjectCreated:Put    │      │   /getFloodEvidence      │      │   (>1.5x width, <0.20 slope)│
  │ • Passes eventId ONLY     │      │ • Issues evidenceToken   │      │ • Proves localized disaster │
  └───────────────────────────┘      └──────────────────────────┘      └──────────────┬──────────────┘
                                                                                      │
  ┌───────────────────────────┐      ┌──────────────────────────┐                     │
  │ 5. IMMUTABLE S3 AUDIT     │      │ 4. DISPATCH ACTION TOOL  │                     │
  │ • Write audit receipt     │ ◄─── │ • Calls /dispatchFloodAlert                     │
  │ • Dedicated audit bucket  │      │ • Validates token &      │ ◄───────────────────┘
  │ • SHA256 integrity seal   │      │   DynamoDB idempotency   │
  └───────────────────────────┘      │ • Dispatches SNS alert   │
                                     └──────────────────────────┘
```

---

## 📊 Special Award Highlights

### 1. Best Use of COOL Award ($1,000)
* **Target Hardware**: AWS Graviton3 (`c7g.2xlarge`, Neoverse V1, Arm64).
* **Software**: Cloud-Optimized OpenCV Library (COOL) with active Arm `NEON_DOTPROD`, `NEON_FP16`, and `SVE` vector extensions.
* **Benchmark Results (100 iterations on full $893 \times 1172$ disaster scene)**:
  * **P50 Latency**: 185.3 ms (vs. 248.5 ms Intel baseline, **-25.4% latency drop**)
  * **Cost Per Scene**: $0.00001685 USD (vs. $0.00002490 USD, **-32.3% cost reduction**)
  * **Throughput per $1.00**: **59,357 scenes / dollar** (+47.8% efficiency advantage)
* **Reproduction Guide**: See [`docs/aws_benchmark_guide.md`](aws_benchmark_guide.md).

### 2. Agentic Vision Award ($1,000)
* **True Autonomous Tool Loop**: The agent is NOT spoon-fed pre-computed metrics. It receives only an event identifier, calls `/getFloodEvidence`, receives a cryptographic `evidenceToken`, verifies mathematical gates against policy, and calls `/dispatchFloodAlert`.
* **Zero Hallucinations & Server-Side Idempotency**: Handled through DynamoDB atomic conditional writes, preventing duplicate emergency broadcasts even on S3 retries or agent re-invocations.
* **CDK v2 Infrastructure Code**: Fully authored in [`aws_infra/`](../aws_infra) (`StorageStack`, `AlertStack`, `IAMStack`, `AgentStack`).

---

## 📑 Core Documentation Directory

* **Turnkey Devpost Submission Copy**: [`docs/devpost_submission.md`](devpost_submission.md)
* **5-Minute Demo Video Storyboard**: [`docs/devpost_submission.md` §2](devpost_submission.md#2-5-minute-demo-video-storyboard--timing)
* **Live Demo Presentation Pack**: [`docs/hackathon_presentation.md`](hackathon_presentation.md)
* **Graviton3 & COOL Benchmark Guide**: [`docs/aws_benchmark_guide.md`](aws_benchmark_guide.md)
* **Nepal Glacier Flood Case Study**: [`docs/flood_nepal_2026.md`](flood_nepal_2026.md)
* **Evidence Export JSON Schema**: [`docs/evidence_schema.md`](evidence_schema.md)
