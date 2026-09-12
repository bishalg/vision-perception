# Dual Hackathon Strategy & Progress Dashboard

> **Target Competitions**:
> 1. **OpenCV AI Competition 2026 (powered by AWS)** — *Deadline: Oct 27, 2026*
> 2. **Nebius x NVIDIA Global AI Hackathon** — *Deadline: Oct 30, 2026*
>
> **Core IP Directive**: The core C++ differential geometry engine (`core/`, `adapters/`, `domains/`) remains **100% Closed Source & Proprietary**. Open source parts are strictly client adapters, public agentic loop orchestrations, and integration schemas.

---

## 1. Executive Comparison & Architectural Duality

| Dimension | OpenCV AI Competition 2026 (AWS) | Nebius x NVIDIA Global AI Hackathon |
| :--- | :--- | :--- |
| **Track / Focus** | Focus Paths: **Agentic Vision Award** & **Best Use of COOL Award** | **Physical AI Track** (Embodied / Edge / Environmental Sensing Agent) |
| **Core Compute & Cloud** | AWS (AWS Graviton, CDK v2, Lambda, S3, DynamoDB, SNS/SQS) | Nebius Token Factory & Nebius AI Cloud (Serverless Endpoints / Jobs) |
| **AI / Model Engine** | Amazon Bedrock (Claude / Titan / custom agent tool loop) | **NVIDIA Nemotron** (Nemotron 3 Ultra / Super / Nano, or Cosmos) |
| **Vision Substrate** | **OpenCV 5** & Cloud-Optimized OpenCV Library (**COOL**) on Graviton | Multi-sensor telemetry & visual frames from river/CCTV/Sentinel |
| **Repo Openness Rule** | Can be **Private / Judge-Accessible** (IP stays safe) | Must be **Public Open Source repo (Apache 2.0 / MIT)** |
| **Open Source Strategy**| Keep submission repo private or share judge link | Provide public **Agent Wrapper / Public Client SDK repo** |
| **Total Cash Prize** | $20,250 ($5,000 Grand + $1,000 COOL + $1,000 Agentic) | $50,000+ ($20,000 Grand + $10,000 2nd + Track Winners Jetson Orins) |
| **Submission Deadline**| **Oct 27, 2026 @ 12:15 PM GMT+5:30** | **Oct 30, 2026 @ 10:30 PM GMT+5:30** |

---

## 2. Architectural Separation: Proprietary Core vs. Dual Hackathon Clients

```text
                           ┌────────────────────────────────────────────────────────┐
                           │   PROPRIETARY & CLOSED SOURCE CORE (vision-perception) │
                           │   • C++20 Scale-Space Steger Ridge Engine              │
                           │   • Sub-pixel Curvilinear Tracking                     │
                           │   • ECC Co-registration (OpenCV 5 / COOL)              │
                           │   • Emits: flood_delta.json / GeoJSON Telemetry        │
                           └───────────────────────────┬────────────────────────────┘
                                                       │
                           ┌───────────────────────────┴────────────────────────────┐
                           │            Standardized Telemetry & Evidence API       │
                           │            (Evidence JSON, Bounding Vectors, Ratios)   │
                           └─────────────┬────────────────────────────┬─────────────┘
                                         │                            │
             ┌───────────────────────────▼────────────┐   ┌───────────▼───────────────────────────┐
             │       TRACK 1: OPENCV AI 2026 (AWS)    │   │  TRACK 2: NEBIUS x NVIDIA GLOBAL AI   │
             │       Private / Judge-Archive          │   │  Public Open-Source Repo (Apache 2.0) │
             ├────────────────────────────────────────┤   ├───────────────────────────────────────┤
             │ • OpenCV 5 pipeline on AWS Graviton    │   │ • Physical AI Edge Agent Orchestrator │
             │ • COOL ARM acceleration benchmarks     │   │ • Nebius Token Factory Client         │
             │ • AWS Bedrock Agentic Decision Loop    │   │ • NVIDIA Nemotron Multi-Step Analysis │
             │ • CDK v2 IaC (S3, DynamoDB, SNS/SQS)   │   │ • Autonomous Civil Defense Dispatcher │
             │ • docs/devpost_submission.md           │   │ • Open Client SDK & Demo Fixtures     │
             └────────────────────────────────────────┘   └───────────────────────────────────────┘
```

---

## 3. Milestones & Progress Tracker

### Phase 1: Shared Core & Telemetry
- [x] Sub-pixel river width & flood boundary calculation (`flood_delta.json`).
- [x] Dual-control false positive suppression (spatial slope + temporal baseline).
- [x] Strict IP containment: root `LICENSE` confirmed Proprietary & Confidential.
- [ ] Unified CLI/API exporter to emit standardized flood evidence JSON for external agent runtimes.

### Phase 2: OpenCV AI Competition 2026 (AWS)
- [x] Draft initial submission documentation (`docs/devpost_submission.md`).
- [x] AWS CDK v2 infrastructure code (`aws_infra/`).
- [x] AWS Bedrock Agentic Vision loop (`getFloodEvidence` $\to$ `dispatchFloodAlert`).
- [ ] Finalize OpenCV 5 + COOL Graviton benchmark reproducibility guide (`docs/aws_benchmark_guide.md`).
- [ ] Record 5-minute judge video demonstration.
- [ ] Submit to OpenCV Devpost before **Oct 27, 2026**.

### Phase 3: Nebius x NVIDIA Global AI Hackathon
- [ ] Register on Devpost & claim Nebius Token Factory credits.
- [ ] Design open-source wrapper repository specification (`river-sentinel-agent`).
- [ ] Implement Nebius Token Factory adapter calling **NVIDIA Nemotron 3** for flood hazard reasoning.
- [ ] Build multi-step decision pipeline: Evidence Ingestion $\to$ Nemotron Validation $\to$ Alert Synthesis.
- [ ] Record 3-minute demo video showing physical application modules in action (CCTV/Sentinel feeds).
- [ ] Publish open-source GitHub repo with Apache 2.0 license & README setup.
- [ ] Submit to Nebius Devpost before **Oct 30, 2026**.

---

## 4. Winning Tactics

1. **For OpenCV + AWS**:
   - Win the **Agentic Vision Award ($1,000)** by showing OpenCV 5 output *actively* altering downstream cloud actions and tool invocations.
   - Win the **Best Use of COOL Award ($1,000)** by demonstrating benchmarked latency/cost reductions on AWS Graviton3.
   - Compete for the **Grand Prize ($5,000)** with real-world Himalayan glacier/flash flood impact.

2. **For Nebius + NVIDIA**:
   - Target the **Physical AI Track ($1 NVIDIA Jetson Orin Nano + Grand Prize contention)** by framing the system as an autonomous physical environmental agent.
   - Highlight deep utilization of **NVIDIA Nemotron** on **Nebius Token Factory** for chain-of-thought verification of physical flood vectors.
   - Satisfy the open-source rule completely without giving away any proprietary C++ core algorithms.

---

## 5. Enforced IP Boundary & Field Redaction Matrix

| Layer / Component | In Public Repo (`river-sentinel-nemotron`) | In Closed-Source Core (`vision-perception`) | Rationale / IP Protection |
| :--- | :---: | :---: | :--- |
| **C++20 Steger Scale-Space Math** | ❌ **Strictly Excluded** | ✅ **Proprietary** | Proprietary differential geometry & sub-pixel Hessian tensor kernels. |
| **Gaussian Derivative Convolutions** | ❌ **Strictly Excluded** | ✅ **Proprietary** | Trade secret core implementation. |
| **Sub-pixel Taylor Zero-Crossing** | ❌ **Strictly Excluded** | ✅ **Proprietary** | Proprietary curve tracing logic. |
| **Internal Hyperparameters ($\sigma$, thresholds)** | ❌ **Redacted** | ✅ **Proprietary** | Prevents reconstruction of algorithmic tuning. |
| **Pointers, C++ Class Names, Mem Offsets** | ❌ **Zero Leakage** | ✅ **Internal** | Clean JSON boundary without memory or class leakages. |
| **Standardized Telemetry (`flood_evidence_v1.json`)** | ✅ **Included (Schema & Fixtures)** | ✅ **Emitted** | Language-neutral observation payload. |
| **NVIDIA Nemotron Agentic Decision Loop** | ✅ **Apache 2.0 Open Source** | ❌ None | Autonomous reasoning, tool calling, and prompt templates. |
| **Civil Defense Alert Dispatch Webhooks** | ✅ **Apache 2.0 Open Source** | ❌ None | Downstream emergency notification integrations. |
| **Synthetic Time-Series Test Streams** | ✅ **Apache 2.0 Open Source** | ✅ Emitted | Mock fixtures for testing client agents without proprietary imagery. |

