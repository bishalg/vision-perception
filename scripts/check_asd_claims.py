#!/usr/bin/env python3
"""Claims and secrets gate for the ASD Gaze app (ADR-0004, CLM-1/5/6, CI-4/5).

This file is the single source of truth for the denylist. The human-readable copy
is docs/spec/modules/asd-gaze-app-store-readiness/claims-denylist.md.

Usage:
  scripts/check_asd_claims.py             scan store metadata (all locales, iOS + Android)
  scripts/check_asd_claims.py --swift     also scan user-facing Swift string literals (CLM-2)
  scripts/check_asd_claims.py --secrets   scan fastlane for committed personal identifiers (CI-4)
  scripts/check_asd_claims.py --selftest  prove the checks fail on seeded violations

Exit status 0 when clean, 1 when any violation is found.
An inline `// claims-ok: <reason>` marker exempts a Swift line (official instrument vocabulary).
"""
from __future__ import annotations

import argparse
import re
import shutil
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
APP = REPO / "apps" / "ios_asd_gaze_app"
METADATA = APP / "fastlane" / "metadata"
FASTLANE = APP / "fastlane"
SWIFT_SOURCES = APP / "Sources"

# --------------------------------------------------------------------------- limits
# Mirrored from the `validate_metadata` lane in fastlane/Fastfile (iOS) and Google Play.
IOS_LIMITS = {
    "name.txt": 30,
    "subtitle.txt": 30,
    "keywords.txt": 100,
    "promotional_text.txt": 170,
    "description.txt": 4000,
    "release_notes.txt": 4000,
}
ANDROID_LIMITS = {
    "title.txt": 30,
    "short_description.txt": 80,
    "full_description.txt": 4000,
}
IOS_LOCALES = ("en-US", "hi", "ne-NP")

# --------------------------------------------------------------------------- denylist
# (id, regex, message). Applied to every metadata text field, case-insensitive.
GLOBAL_DENY = [
    ("test", r"\btests?\b", '"test" as a noun describing the app'),
    ("fda", r"\bFDA\b", "FDA claim"),
    ("hipaa", r"\bHIPAA\b", "HIPAA claim"),
    ("irb", r"\bIRB\b", "IRB claim (no research study at launch, H6)"),
    ("protocol", r"\bprotocol\s*#", "study protocol number"),
    ("clin-valid", r"clinically\s+validated", "clinical validation claim"),
    ("clinical", r"\bclinical(ly)?\b", "clinical wording (ADR-0004 decision 7)"),
    ("biomarker", r"bio-?markers?", "biomarker wording"),
    ("adhd", r"\bADHD\b", "condition name ADHD"),
    ("autism-screen", r"autism\s+(screening|screener|test)", "autism screening/test claim"),
    ("screening", r"\bscreen(ing|er)s?\b", "screening wording"),
    ("risk-score", r"risk\s+(score|percentage|stratification)", "risk score claim"),
    ("bayesian", r"\bbayesian\b", "Bayesian claim"),
    ("probability", r"\bprobabilit(y|ies)\b", "probability claim"),
    ("smart-fhir", r"SMART\s+on\s+FHIR", "SMART on FHIR claim"),
    ("ane-facs", r"\b(ANE|FACS)\b", "technical jargon (ADR-0004 decision 7)"),
    ("study", r"\b(research\s+study|principal\s+investigator)\b", "research study claim (H6)"),
    ("astro-fusion", r"astro-fusion\.com", "URL not controlled by the publisher (CI-5)"),
    # Hindi / Nepali equivalents (Devanagari)
    ("hi-biomarker", r"बायोमार्कर|बायोमार्कर्स", "biomarker wording (hi/ne)"),
    ("hi-screening", r"स्क्रीनिंग|स्क्रिनिङ|स्क्रीनर", "screening wording (hi/ne)"),
    ("hi-adhd", r"एडीएचडी", "condition name ADHD (hi/ne)"),
    ("hi-test", r"परीक्षण", "test wording (hi/ne)"),
    ("hi-clinical", r"नैदानिक|क्लिनिकल|क्लिनिकली", "clinical wording (hi/ne)"),
]

# Stricter: name, subtitle, keywords, title never mention the condition at all (ADR-0004 d1).
STRICT_FIELDS = {"name.txt", "subtitle.txt", "keywords.txt", "title.txt", "short_description.txt"}
STRICT_DENY = [
    ("autism", r"\bautis(m|tic)\b", "condition name in name/subtitle/keywords"),
    ("diagnos", r"diagnos", "diagnosis wording in name/subtitle/keywords"),
    ("hi-autism", r"ऑटिज़्म|ऑटिज्म|अटिजम|अटिज्म|ऑटिस्म|अटिजमको", "condition name in name/subtitle/keywords (hi/ne)"),
    ("hi-diagnos", r"निदान", "diagnosis wording in name/subtitle/keywords (hi/ne)"),
]

# "diagnos*" is allowed only inside a sentence that says the app does not diagnose.
DIAGNOS = re.compile(r"diagnos|निदान", re.IGNORECASE)
NEGATION = re.compile(
    r"\b(not|no|never|cannot|without|nor)\b|n't|नहीं|होइन|गर्दैन|नहीं करता",
    re.IGNORECASE,
)

MANDATORY = ("This is not a diagnostic tool.", "Part of Open Spectrum Tracking")
CATEGORY = "HEALTH_AND_FITNESS"  # H5 default; changing it is an ADR-0005 decision
URL_FILES = (
    "age_suitability_url.txt",
    "marketing_url.txt",
    "support_url.txt",
    "privacy_url.txt",
    "privacy_policy_url.txt",
)

# --------------------------------------------------------------------------- secrets
SECRET_PATTERNS = [
    ("phone", r"\+?977[-\s]?98\d{8}|9802030896"),
    ("personal-email", r"bishal\.ghimire@gmail|binodprasadghimire|doctor\.review@"),
    ("demo-password", r"ClinicalPass2026"),
    ("team-id", r"\b6J3WY94J6T\b"),
    ("team-name", r"Binod Ghimire|Bishal Ghimire"),
    ("literal-apple-id", r"""apple_id\s*[:(]?\s*["'][^"'\s]+@[^"'\s]+["']"""),
    ("literal-email", r"""(?<![\w.])[\w.+-]+@(gmail|yahoo|outlook|hotmail|icloud)\.com"""),
]
SECRET_SKIP_DIRS = {"__pycache__", "report", "screenshots"}


@dataclass
class Finding:
    where: str
    rule: str
    detail: str

    def __str__(self) -> str:
        return f"{self.where}: [{self.rule}] {self.detail}"


# --------------------------------------------------------------------------- checks
def _sentences(text: str) -> list[str]:
    return [s for s in re.split(r"[.!?।\n•]+", text) if s.strip()]


def check_text(where: str, field: str, text: str) -> list[Finding]:
    out: list[Finding] = []
    for rid, rx, msg in GLOBAL_DENY:
        m = re.search(rx, text, re.IGNORECASE)
        if m:
            out.append(Finding(where, rid, f'{msg}: "{m.group(0)}"'))
    if field in STRICT_FIELDS:
        for rid, rx, msg in STRICT_DENY:
            m = re.search(rx, text, re.IGNORECASE)
            if m:
                out.append(Finding(where, rid, f'{msg}: "{m.group(0)}"'))
    elif DIAGNOS.search(text):
        for s in _sentences(text):
            if DIAGNOS.search(s) and not NEGATION.search(s):
                out.append(Finding(where, "diagnos", f'"diagnos*" outside a negating sentence: "{s.strip()[:80]}"'))
    return out


def _read(p: Path) -> str | None:
    return p.read_text(encoding="utf-8").strip() if p.is_file() else None


def check_metadata(root: Path) -> list[Finding]:
    out: list[Finding] = []
    targets: list[tuple[Path, dict[str, int], str]] = []
    for loc in IOS_LOCALES:
        targets.append((root / loc, IOS_LIMITS, f"ios/{loc}"))
    targets.append((root / "android" / "en-US", ANDROID_LIMITS, "android/en-US"))

    for d, limits, label in targets:
        if not d.is_dir():
            out.append(Finding(label, "missing", "locale directory not found"))
            continue
        for f in sorted(d.glob("*.txt")):
            text = f.read_text(encoding="utf-8").strip()
            where = f"{label}/{f.name}"
            out += check_text(where, f.name, text)
            lim = limits.get(f.name)
            if lim is not None and len(text) > lim:
                out.append(Finding(where, "length", f"{len(text)} > {lim} chars"))
        # mandatory sentences on the store description
        desc = _read(d / ("full_description.txt" if label.startswith("android") else "description.txt"))
        if desc is None:
            out.append(Finding(label, "missing", "description file not found"))
        else:
            for s in MANDATORY:
                if s not in desc:
                    out.append(Finding(label, "mandatory", f'description lacks "{s}" (CLM-5)'))
        for req in (("title.txt",) if label.startswith("android") else ("name.txt",)):
            if not _read(d / req):
                out.append(Finding(label, "missing", f"{req} missing or empty"))

    cat = _read(root / "en-US" / "primary_category.txt")
    if cat != CATEGORY:
        out.append(Finding("ios/en-US/primary_category.txt", "category", f"{cat!r}, expected {CATEGORY} (CLM-6)"))

    for u in URL_FILES:
        v = _read(root / "en-US" / u)
        if v is None:
            continue
        if not re.fullmatch(r"https://[^\s/]+\.[^\s/]+(/\S*)?", v):
            out.append(Finding(f"ios/en-US/{u}", "url", f"blank or not an https URL: {v!r} (CI-5)"))

    # copyright files must not carry a personal name
    for c in [root / "copyright.txt", root / "en-US" / "copyright.txt"]:
        v = _read(c)
        if v and re.search(r"Ghimire", v):
            out.append(Finding(str(c.relative_to(root)), "personal-name", "copyright holder is a person"))
    return out


_LITERAL = re.compile(r'"((?:[^"\\]|\\.)*)"')
_UI_CALL = re.compile(r"\b(Text|Label|String\(format:|NSLocalizedString|Button|navigationTitle|\.help|alert|\.draw|Localizer|L\()")
_SKIP_LINE = re.compile(r"^\s*(//|///|\*)")


def check_swift(root: Path) -> list[Finding]:
    out: list[Finding] = []
    for f in sorted(root.rglob("*.swift")):
        for n, line in enumerate(f.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            if _SKIP_LINE.match(line) or "claims-ok:" in line:
                continue
            code = line.split("//")[0] if "//" in line and '"' not in line.split("//")[0][-1:] else line
            if not _UI_CALL.search(code):
                continue
            for lit in _LITERAL.findall(code):
                if len(lit) < 4:
                    continue
                for fd in check_text(f"{f.name}:{n}", "ui", lit):
                    # "test" and URL-ish rules are noisy in code literals
                    if fd.rule not in {"test", "astro-fusion", "study", "ane-facs"}:
                        out.append(fd)
    return out


def check_secrets(fastlane: Path, metadata: Path | None = None) -> list[Finding]:
    out: list[Finding] = []
    review = fastlane / "metadata" / "review_information"
    if review.exists() and any(review.iterdir()):
        out.append(Finding(str(review.relative_to(fastlane.parent)), "review-information",
                           "directory still holds review contact data (WP-1.6 deletes it, CI-4)"))
    for f in sorted(fastlane.rglob("*")):
        if not f.is_file() or SECRET_SKIP_DIRS & set(f.parts):
            continue
        try:
            text = f.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for rid, rx in SECRET_PATTERNS:
            m = re.search(rx, text)
            if m:
                out.append(Finding(str(f.relative_to(fastlane.parent)), rid, f'matches "{m.group(0)}"'))
    return out


# --------------------------------------------------------------------------- selftest
GOOD = {
    "en-US/name.txt": "EarlyGaze Observation Diary",
    "en-US/subtitle.txt": "Engagement and daily notes",
    "en-US/keywords.txt": "diary,toddler,development,attention,gaze,milestones,mood,parent,routine",
    "en-US/promotional_text.txt": "A private diary of your child's attention and daily moments. Camera frames are never stored.",
    "en-US/description.txt": "A diary for parents.\n\nThis is not a diagnostic tool.\nPart of Open Spectrum Tracking.\n",
    "en-US/release_notes.txt": "Observation diary with engagement notes.",
    "en-US/primary_category.txt": "HEALTH_AND_FITNESS",
    "en-US/marketing_url.txt": "https://example.org/asd-gaze",
    "en-US/support_url.txt": "https://example.org/support",
    "en-US/privacy_url.txt": "https://example.org/privacy",
    "en-US/copyright.txt": "© 2026 Example Org.",
    "hi/name.txt": "अर्लीगेज अवलोकन डायरी",
    "hi/description.txt": "माता-पिता के लिए डायरी। यह निदान नहीं करता।\nThis is not a diagnostic tool.\nPart of Open Spectrum Tracking.",
    "ne-NP/name.txt": "अर्लीगेज अवलोकन डायरी",
    "ne-NP/description.txt": "अभिभावकका लागि डायरी। यो निदान होइन।\nThis is not a diagnostic tool.\nPart of Open Spectrum Tracking.",
    "android/en-US/title.txt": "EarlyGaze Observation Diary",
    "android/en-US/short_description.txt": "A parent's diary of attention and daily moments.",
    "android/en-US/full_description.txt": "A diary.\nThis is not a diagnostic tool.\nPart of Open Spectrum Tracking.",
}

# (label, file, replacement-or-callable producing new text, expected rule). Text replaced wholesale.
BAD = [
    ("name with Test", "en-US/name.txt", "EarlyGaze Autism Test", "test"),
    ("name with autism", "en-US/name.txt", "EarlyGaze Autism Diary", "autism"),
    ("subtitle too long", "en-US/subtitle.txt", "x" * 31, "length"),
    ("keywords biomarker", "en-US/keywords.txt", "diary,biomarker", "biomarker"),
    ("keywords screening", "en-US/keywords.txt", "diary,screening", "screening"),
    ("keywords too long", "en-US/keywords.txt", "a," * 51, "length"),
    ("promo risk score", "en-US/promotional_text.txt", "See your risk score.", "risk-score"),
    ("release notes FDA", "en-US/release_notes.txt", "FDA-grade diary", "fda"),
    ("release notes HIPAA", "en-US/release_notes.txt", "HIPAA aligned", "hipaa"),
    ("release notes IRB", "en-US/release_notes.txt", "IRB Protocol #2026-088", "irb"),
    ("description clinically validated", "en-US/description.txt",
     "Clinically validated.\nThis is not a diagnostic tool.\nPart of Open Spectrum Tracking.", "clin-valid"),
    ("description Bayesian", "en-US/description.txt",
     "Bayesian.\nThis is not a diagnostic tool.\nPart of Open Spectrum Tracking.", "bayesian"),
    ("description diagnoses un-negated", "en-US/description.txt",
     "It diagnoses autism early.\nThis is not a diagnostic tool.\nPart of Open Spectrum Tracking.", "diagnos"),
    ("description lacks mission line", "en-US/description.txt", "This is not a diagnostic tool.", "mandatory"),
    ("description lacks disclaimer", "en-US/description.txt", "Part of Open Spectrum Tracking.", "mandatory"),
    ("category MEDICAL", "en-US/primary_category.txt", "MEDICAL", "category"),
    ("url astro-fusion", "en-US/support_url.txt", "https://astro-fusion.com", "astro-fusion"),
    ("url blank", "en-US/privacy_url.txt", "", "url"),
    ("url not https", "en-US/marketing_url.txt", "http://example.org", "url"),
    ("copyright personal name", "en-US/copyright.txt", "© 2026 Binod Ghimire", "personal-name"),
    ("hi name screening", "hi/name.txt", "अर्लीगेज स्क्रीनिंग", "hi-screening"),
    ("hi name autism", "hi/name.txt", "अर्लीगेज ऑटिज़्म डायरी", "hi-autism"),
    ("ne description biomarker", "ne-NP/description.txt",
     "बायोमार्कर्स\nThis is not a diagnostic tool.\nPart of Open Spectrum Tracking.", "hi-biomarker"),
    ("ne description lacks sentences", "ne-NP/description.txt", "डायरी", "mandatory"),
    ("android title test", "android/en-US/title.txt", "EarlyGaze: Autism & ADHD Test", "test"),
    ("android title too long", "android/en-US/title.txt", "y" * 31, "length"),
    ("android short too long", "android/en-US/short_description.txt", "z" * 81, "length"),
    ("android full ADHD", "android/en-US/full_description.txt",
     "ADHD\nThis is not a diagnostic tool.\nPart of Open Spectrum Tracking.", "adhd"),
]


def _write_tree(root: Path, files: dict[str, str]) -> None:
    for rel, text in files.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8")


def selftest() -> int:
    failures: list[str] = []
    tmp = Path(tempfile.mkdtemp(prefix="claims-selftest-"))
    try:
        base = tmp / "good"
        _write_tree(base, GOOD)
        got = check_metadata(base)
        if got:
            failures.append("good fixture should pass but produced: " + "; ".join(map(str, got)))

        for label, rel, text, rule in BAD:
            d = tmp / "bad"
            if d.exists():
                shutil.rmtree(d)
            _write_tree(d, {**GOOD, rel: text})
            rules = {f.rule for f in check_metadata(d)}
            if rule not in rules:
                failures.append(f"{label}: expected rule {rule!r}, got {sorted(rules)}")

        # swift scan
        sw = tmp / "swift"
        _write_tree(sw, {
            "ok.swift": 'Text("Engagement flag")\nlet identifier = "biomarkerKey"\n// Text("biomarker")\n',
            "okmark.swift": 'Text("High Risk probability") // claims-ok: M-CHAT-R/F official category\n',
        })
        if check_swift(sw):
            failures.append("swift good fixture should pass: " + "; ".join(map(str, check_swift(sw))))
        for label, line, rule in [
            ("swift biomarker Text", 'Text("16 digital biomarkers")', "biomarker"),
            ("swift risk score", 'Label("Your risk score", systemImage: "x")', "risk-score"),
            ("swift Elevated probability", 'Text("Calibrated Bayesian risk probability")', "bayesian"),
        ]:
            d = tmp / "swiftbad"
            if d.exists():
                shutil.rmtree(d)
            _write_tree(d, {"v.swift": line + "\n"})
            if rule not in {f.rule for f in check_swift(d)}:
                failures.append(f"{label}: expected {rule!r}")

        # secrets
        fl = tmp / "fastlane"
        _write_tree(fl, {"Appfile": 'app_identifier ENV["APP_IDENTIFIER"]\napple_id ENV["APPLE_ID"]\n'})
        if check_secrets(fl):
            failures.append("clean fastlane should pass: " + "; ".join(map(str, check_secrets(fl))))
        for label, rel, text, rule in [
            ("secret apple id", "Appfile", 'apple_id "someone@yahoo.com"', "literal-apple-id"),
            ("secret team id", "config.yml", 'team_id: "6J3WY94J6T"', "team-id"),
            ("secret phone", "metadata/notes.txt", "call +977-9802030896", "phone"),
            ("secret password", "x.txt", "ClinicalPass2026!", "demo-password"),
            ("secret review dir", "metadata/review_information/phone_number.txt", "123", "review-information"),
        ]:
            d = tmp / "fl-bad"
            if d.exists():
                shutil.rmtree(d)
            _write_tree(d / "fastlane", {rel: text})
            if rule not in {f.rule for f in check_secrets(d / "fastlane")}:
                failures.append(f"{label}: expected {rule!r}")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    if failures:
        print("SELFTEST FAILED")
        for f in failures:
            print("  -", f)
        return 1
    print(f"selftest ok: {len(BAD)} metadata, 3 swift, 5 secrets seeded failures all detected")
    return 0


# --------------------------------------------------------------------------- main
def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--selftest", action="store_true", help="run seeded-failure self test and exit")
    ap.add_argument("--secrets", action="store_true", help="scan fastlane for personal identifiers and secrets")
    ap.add_argument("--swift", action="store_true", help="also scan user-facing Swift string literals")
    args = ap.parse_args()

    if args.selftest:
        return selftest()

    findings: list[Finding] = []
    if args.secrets:
        findings += check_secrets(FASTLANE)
    else:
        findings += check_metadata(METADATA)
        if args.swift:
            findings += check_swift(SWIFT_SOURCES)

    for f in findings:
        print(f)
    if findings:
        print(f"\n{len(findings)} violation(s).")
        return 1
    print("claims check ok" if not args.secrets else "secrets check ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
