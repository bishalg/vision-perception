# OpenCV AI Competition 2026, Powered by AWS — Reference & Checklist

> **Devpost Link**: [OpenCV AI Competition 2026](https://opencv26.devpost.com/)  
> **Final Submissions Due**: **October 26, 2026 @ 11:59 PM PDT** *(October 27, 2026 @ 12:44 PM NPT)*  
> **Judging Period**: **October 27 – November 9, 2026**  
> **Winners Announced**: **November 10, 2026** (on OpenCV Live!)  
> **Total Prizes**: **$12,000+ in cash awards** ($5k 1st, $3k 2nd, $2k 3rd, $1k Best Use of COOL, $1k Agentic Vision)  
> **IP / Repo Visibility**: **Can be Private / Judge-Accessible** (No requirement to make source public).

---

## 1. Prize Structure
- **First Place Grand Prize**: $5,000 cash (1 winner)
- **Second Place**: $3,000 cash (1 winner)
- **Third Place**: $2,000 cash (1 winner)
- **Best Use of COOL Award**: $1,000 cash (1 winner)
- **Agentic Vision Award**: $1,000 cash (1 winner)
- **Compute Grant for AWS**: $150 in credits (55 winners)

---

## 2. Judging Criteria (100% Total)
1. **Technical Execution (30%)**: Correctness and depth of OpenCV 5 usage.
2. **Innovation (20%)**: Originality and thoughtful use of computer vision or AI.
3. **Real-world Impact (20%)**: Importance of the problem, usefulness, and evidence of potential benefit.
4. **User Experience (10%)**: Usability, accessibility, and quality of interaction.
5. **Documentation & Presentation (10%)**: Clarity of report, architecture diagrams, and video.
6. **Cloud Delivery & Responsible Operation (10%)**: AWS deployment quality, repeatability, Graviton/COOL efficiency, and responsible use practices.

---

## 3. Key Requirements Checklist
- [x] Substantive image/video analysis using **OpenCV 5**. *(41/41 ctest green, Steger RMSE 0.0301 px, flood pipeline verified 2026-10-24)*
- [x] Run meaningful component on **AWS** (Graviton, S3, DynamoDB, Bedrock, etc.). *(CDK v2 IaC authored + **offline `cdk synth` verified 2026-10-24** — all 4 stack templates generate after fixing a Storage↔Agent dependency cycle; 11/11 agent tests green; credential-free local 3-node trace verified; live deploy runbook in `aws_infra/README.md`)*
- [x] Target **Agentic Vision Path**: Vision evidence directly drives downstream tools/actions (`getFloodEvidence` $\to$ `dispatchFloodAlert`). *(both directions evidenced: `--scenario critical` dispatches, `--scenario marginal` records a NOT_DISPATCHED standby — the OpenCV 5 output gates the action)*
- [x] Target **Best Use of COOL Path** — prepared: harness + measured 100-iteration Arm64 run (P50 213.6 ms, 50,744 scenes/$1) + EC2 reproduction guide. **⚠ AWARD ELIGIBILITY (rules): "COOL must execute the claimed core workload on AWS Graviton" — the c7g.2xlarge run in `docs/aws_benchmark_guide.md` must happen before submission; local Arm64 alone does not satisfy it.**
- [x] Technical Report describing problem, architecture, AWS deployment, and limitations. *(docs/flood_nepal_2026.md)*
- [x] Architecture Diagram showing OpenCV 5, AWS components, and agentic loop. *(docs/assets/hackathon_architecture.png — created 2026-10-24, SVG + reproducible renderer)*
- [ ] Demo Video: Public or unlisted judge-accessible video ($\le$ 5 minutes). **→ record next via `docs/VIDEO_RECORDING_RUNBOOK.md`**
- [x] Working web endpoint or live screen-share demonstration. *(credential-free judges path: validator + agentic demo + flood pipeline, all verified 2026-10-24)*
- [ ] Private/judge-accessible code repository with build/run instructions. *(README + docs/JUDGES_QUICKSTART.md ready — grant judges access when filing)*
