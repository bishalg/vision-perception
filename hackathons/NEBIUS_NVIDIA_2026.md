# Nebius x NVIDIA Global AI Hackathon — Reference & Checklist

> **STATUS: NOT PURSUED (Decision H-1, 2026-10-27)** — Team focus dedicated to Phase 1 ASD deliverables, backend, and core platform hygiene. See `docs/roadmap_2026q4/00_repo_state_and_urgent_actions.md`.
>
> **Devpost Link**: [Nebius x NVIDIA Global AI Hackathon](https://nebiusglobalaihackathon.devpost.com/)  
> **Deadline**: **October 30, 2026 @ 10:30 PM GMT+5:30**  
> **Total Prizes**: **$50,000+ in cash + NVIDIA Hardware**  
> **Target Track**: **Physical AI Track**  
> **IP / Repo Visibility**: **Must be a public code repository with an open source license (Apache 2.0 / MIT)**.

---

## 1. Prize Structure
- **Grand Prize**: $20,000 cash (1 winner)
- **2nd Place**: $10,000 cash (1 winner)
- **3rd Place**: $6,000 cash (1 winner)
- **Physical AI Track Winner**: 1 NVIDIA Jetson Orin Nano
- **Best Apps and Agents Track Winner**: 1 NVIDIA Jetson Orin Nano
- **Coding and Agentic Engineering Track Winner**: 1 NVIDIA Jetson Orin Nano
- **Personal AI Track Winner**: 1 NVIDIA Jetson Orin Nano
- **Best Use of Tavily**: $3,000 cash
- **City Winner Awards**: $500 cash (20 winners)
- **Most Valuable Feedback**: $100 cash (10 winners) + NVIDIA swag

---

## 2. Track Selection: Physical AI Track
> *"Build embodied and edge agents that sense and act in the real world: robotics, IoT, and on-device intelligence, powered by Nemotron, GROOT, Cosmos and Sonic models and coordinated through an agent runtime. Use Nebius Serverless Jobs to run simulations, generate synthetic data, evaluate robot policies, or process sensor data at scale, and Nebius Serverless Endpoints to serve real-time inference when needed. Your demonstration video must include at least a 1-minute clip showing the physical hardware/robot actually operating, or — if your project has no physical hardware component — the key application modules in action."*

---

## 3. Open Source & IP Strategy
- **Closed-Source Core (`vision-perception`)**:
  - Proprietary C++20 differential geometry engine, Steger scale-space kernels, and raw math algorithms remain private.
- **Open-Source Submission Repo**:
  - Dedicated public repository (e.g. `flow-sentinel-nemotron` / `river-guard-agent`).
  - License: Apache 2.0.
  - Contains:
    1. Python/TypeScript client integrating with **Nebius Token Factory**.
    2. **NVIDIA Nemotron** agentic reasoning loop for physical disaster / river telemetry evaluation.
    3. Action dispatchers (Civil defense alerts, flood boundary polygon GeoJSON handlers).
    4. Demonstration test datasets and runnable CLI/endpoint.

---

## 4. Judging Criteria
1. **Technological Implementation**: How well the project is built, and how effectively it uses Nebius Token Factory / AI Cloud and NVIDIA Nemotron.
2. **Design**: Complete, coherent product experience (not just an isolated proof of concept).
3. **Potential Impact**: Credible, specific case for solving a real problem for a real audience.
4. **Quality of the Idea**: Creative, non-obvious use of Nebius and NVIDIA Nemotron showing genuine domain understanding.

---

## 5. Submission Checklist
- [ ] Project running on **Nebius Token Factory** or **Nebius AI Cloud**.
- [ ] Utilizing at least one **NVIDIA open source model** (e.g., **NVIDIA Nemotron 3** Ultra/Super/Nano).
- [ ] Category chosen: **Physical AI Track**.
- [ ] Public code repository URL with Apache 2.0 or MIT license and setup instructions.
- [ ] Working demo URL / hosted application / test build.
- [ ] Demo Video: Public YouTube video $\le$ 3 minutes with audio explaining Nebius + NVIDIA Nemotron usage, with $\ge$ 1 minute showing physical application modules in action.
- [ ] Pre-existing project disclosure: Note that the vision sensing engine was existing core research, while the Nebius + NVIDIA Nemotron Physical AI agentic reasoning system was built during the hackathon period.
- [ ] Constructive feedback on Nebius Token Factory and NVIDIA technologies.
