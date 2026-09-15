// OpenCV AI Competition 2026 & Hackathon Submission Menu
export const title = '[HACKATHON] OpenCV AI 2026 & AWS Submission';
export const items = [
  { label: '[SUBMISSION] Validate Full Hackathon Readiness (All 8 Gates)', run: './scripts/validate_hackathon_readiness.sh' },
  { label: '[DEMO] Run Interactive Bedrock Agentic Vision Loop (Local Demo)', run: 'python3 scripts/run_agentic_vision_demo.py' },
  { label: '[PIPELINE] Run C++20 Curvilinear Flood Pipeline (PRE + POST Steger)', run: './scripts/run_flood_pipeline.sh' },
  { label: '[EVIDENCE] Compute Flood Delta & Cryptographic Token', run: 'python3 scripts/compute_flood_delta.py' },
  { label: '[TEST] Run Full End-to-End Pipeline Integration Test', run: 'python3 -m pytest tests/test_e2e_hackathon_pipeline.py -v' },
  { label: '[AGENT] Run AWS Bedrock & DynamoDB Test Suite', run: 'python3 -m pytest aws_infra/tests/' },
  { label: '[BENCHMARK] Run COOL vs x86 Benchmark Harness (Dry Run)', run: './scripts/benchmark_cool_vs_x86.sh --dry-run --skip-build' },
  { label: '[TWIN] Launch Cesium 3D Digital Twin (Vite Dev Server)', run: 'pnpm --dir apps/cesium_viewer dev' },
  { label: '[GUIDE] View Judges & Reviewers Quickstart Guide', run: 'cat docs/JUDGES_QUICKSTART.md | less' },
  { label: '[DOCS] View Devpost Submission Text & Video Script', run: 'cat docs/devpost_submission.md | less' },
];
