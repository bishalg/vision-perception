# ASD Rust Core and Backend - Non-Technical Summary

## What changes

The numbers the app shows a parent (looking-time scores, the green/yellow/red flag,
quality of a recording) are today computed by code that exists only inside the iPhone
app. The reference norms are built by a separate Python script, and the server that
should receive sessions was deleted. This work puts all of that arithmetic in one place,
written in Rust, and has the iPhone app, the clinic console and the server all call the
same code. The server itself is rebuilt in Rust.

## Why it matters

- One implementation means a session scored on the phone, re-scored on the server and
  shown in the console cannot disagree.
- Configuration (which clips, which age norms, which thresholds) is signed by the server
  and verified by the app, so nobody can change scoring silently.
- The same core will run on Android and in the browser without being rewritten.

## What does not change

- The app remains an observation and screening aid, not a diagnostic tool (ADR-0004).
- No raw video is stored anywhere; the server stores pseudonymous summaries only
  (02 ADR-B3, ADR-B8).
- The palm, flood and C++ parts of the repository are untouched.

## Decisions the owner still has to make

| Id | Question | Default if undecided |
| --- | --- | --- |
| H-11 | Infrastructure-as-code language for the AWS deployment (`aws_infra/asd/`): TypeScript CDK (recommended, no new Python) or extend the existing Python CDK app. | TypeScript CDK |
| H-12 | Ratify the `cargo-deny` licence allow-list additions: ISC, Unicode-3.0 (permissive, not yet on the greenlist), OpenSSL licence for the TLS crate, MPL-2.0 for UniFFI only. | Accept all four |
| H-13 | Accept UniFFI (MPL-2.0) in the app binary, or require the C-ABI fallback. | UniFFI |
| H-14 | Demo hosting: Cloudflare Tunnel from a laptop (free, fragile) vs a small VPS running the compose file. | Tunnel for the 2026-11 demo, VPS for the pilot |
| H-15 | Keep the care-events endpoint (CareKit sync from the app) in Phase 1 or defer to Phase 2. | Defer (WP-B.6 in Phase 2) |
