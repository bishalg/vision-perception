# Claims denylist (human-readable copy)

The source of truth is `scripts/check_asd_claims.py`. If this page and the script disagree, the script wins; update this page in the same change.

Run: `python3 scripts/check_asd_claims.py` (metadata), `--swift` (also user-facing Swift string literals, CLM-2), `--secrets` (CI-4), `--selftest` (seeded failures).

## Forbidden in every store text field (all locales, iOS and Android)

| Rule id | Matches (case-insensitive) |
| --- | --- |
| test | "test", "tests" as words |
| fda, hipaa, irb | FDA, HIPAA, IRB |
| protocol | "protocol #" (study protocol numbers) |
| clin-valid, clinical | "clinically validated"; "clinical", "clinically" |
| biomarker | biomarker(s), bio-marker(s) |
| adhd | ADHD |
| autism-screen | "autism screening/screener/test" |
| screening | screening, screener(s) |
| risk-score | "risk score/percentage/stratification" |
| bayesian, probability | Bayesian; probability/probabilities |
| smart-fhir | "SMART on FHIR" |
| ane-facs | ANE, FACS |
| study | "research study", "principal investigator" (no study at launch, H6) |
| astro-fusion | astro-fusion.com (URL not controlled by the publisher, CI-5) |
| hi-* | Devanagari equivalents of biomarker, screening, ADHD, test, clinical |

## Additionally forbidden in name, subtitle, keywords, Android title and short description

autism / autistic, "diagnos*", and the Devanagari equivalents of autism and diagnosis (ADR-0004 decision 1).

## "diagnos*" elsewhere

Allowed only inside a sentence that negates it ("does not diagnose", "not a diagnostic tool"; Hindi "नहीं", Nepali "होइन"/"गर्दैन").

## Required

- Every description (en-US, hi, ne-NP, Android full) contains "This is not a diagnostic tool." and "Part of Open Spectrum Tracking" (CLM-5).
- `primary_category.txt` is `HEALTH_AND_FITNESS` (CLM-6; changing it is an ADR-0005 decision).
- URL files are https URLs, not blank (CI-5).
- Length limits mirror the Fastfile `validate_metadata` lane: name 30, subtitle 30, keywords 100, promotional text 170, description 4000, release notes 4000; Android title 30, short description 80, full description 4000.
- Copyright files carry no personal name.

## Swift sources (`--swift`)

String literals on lines that call `Text(`, `Label(`, `String(format:`, `NSLocalizedString`, `Button`, `Localizer`, `.draw` etc. are checked against the global list (except test, study, ane-facs, astro-fusion). Comments and identifiers are exempt. A line with `// claims-ok: <reason>` is exempt (official M-CHAT-R/F vocabulary only).

## Secrets (`--secrets`)

Fails if `fastlane/metadata/review_information/` is non-empty or any fastlane file contains the maintainer's phone, personal emails, demo password, Apple team id or name, a literal `apple_id`, or a free-mail address.
