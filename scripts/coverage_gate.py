#!/usr/bin/env python3
"""Coverage gate for the vision-perception monorepo.

Fails (exit 1) unless every in-scope source file reaches 100% line coverage
AND 100% branch coverage, as reported by `llvm-cov export` JSON.

Usage:
    coverage_gate.py <export.json> [--scope <scope.txt>] [--exemptions <exemptions.txt>]
                     [--summary <out.json>] [--root <repo-root>]

Scope rules (scripts/coverage_scope.txt):
    - one glob per line, '#' starts a comment, blank lines ignored
    - lines starting with '!' are exclusion patterns
    - paths are repo-relative POSIX; '**' crosses directories, '*' does not

Exemptions (scripts/coverage_exemptions.txt):
    <repo-relative-path>|<justification>
    A file listed here is skipped by the gate. Exemptions are for provably
    unreachable defensive code only; every entry must carry a written
    justification and is surfaced in the gate output.

Semantics:
    - line coverage:    summary.lines.covered == summary.lines.count
    - branch coverage:  summary.branches.covered == summary.branches.count
      (llvm-cov counts a branch as covered only when BOTH edges are taken)
    - an in-scope .cpp/.cc/.cxx file that is absent from the report fails the
      gate (it was never linked into any test binary); absent headers pass
      (no executable code was instantiated).
"""

import argparse
import fnmatch
import json
import re
import sys
from pathlib import Path

SRC_EXTENSIONS = {".cpp", ".cc", ".cxx", ".hpp", ".hh", ".hxx"}


def load_rule_file(path):
    rules = []
    if not path.exists():
        return rules
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        rules.append(line)
    return rules


def glob_to_regex(pattern):
    """Convert a repo-relative glob to a regex. '**' crosses '/', '*' does not."""
    i = 0
    out = []
    while i < len(pattern):
        ch = pattern[i]
        if ch == "*":
            if i + 1 < len(pattern) and pattern[i + 1] == "*":
                out.append(".*")
                i += 2
                # consume a slash following '**' so 'a/**/b' also matches 'a/b'
                if i < len(pattern) and pattern[i] == "/":
                    i += 1
                continue
            out.append("[^/]*")
            i += 1
            continue
        if ch == "?":
            out.append("[^/]")
        elif ch in ".+^$()[]{}|\\":
            out.append("\\" + ch)
        else:
            out.append(ch)
        i += 1
    return re.compile("^" + "".join(out) + "$")


class Scope:
    def __init__(self, rules):
        self.includes = [glob_to_regex(r) for r in rules if not r.startswith("!")]
        self.excludes = [glob_to_regex(r[1:]) for r in rules if r.startswith("!")]

    def matches(self, rel):
        if not any(rx.match(rel) for rx in self.includes):
            return False
        return not any(rx.match(rel) for rx in self.excludes)

    def has_includes(self):
        return bool(self.includes)


def rel_path(filename, root):
    """Normalize an llvm-cov filename to a repo-relative POSIX path."""
    p = Path(filename).resolve()
    try:
        return p.relative_to(root).as_posix()
    except ValueError:
        return p.as_posix()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("export_json", type=Path, help="llvm-cov export JSON path")
    parser.add_argument("--scope", type=Path, default=Path(__file__).parent / "coverage_scope.txt")
    parser.add_argument(
        "--exemptions", type=Path, default=Path(__file__).parent / "coverage_exemptions.txt"
    )
    parser.add_argument("--summary", type=Path, default=None, help="write a summary JSON here")
    parser.add_argument("--root", type=Path, default=Path(__file__).parent.parent)
    args = parser.parse_args()
    args.root = args.root.resolve()

    rules = load_rule_file(args.scope)
    if not rules:
        print("coverage-gate: FATAL scope file is empty: {}".format(args.scope))
        return 2
    scope = Scope(rules)

    exemptions = {}
    for entry in load_rule_file(args.exemptions):
        parts = entry.split("|", 1)
        if len(parts) != 2:
            print("coverage-gate: FATAL malformed exemption entry: {}".format(entry))
            return 2
        exemptions[parts[0].strip()] = parts[1].strip()

    with args.export_json.open("r", encoding="utf-8") as fh:
        data = json.load(fh)

    # Aggregate per-file summaries (multiple objects may contribute the same file).
    files = {}
    for datum in data.get("data", []):
        for entry in datum.get("files", []):
            rel = rel_path(entry["filename"], args.root)
            if not scope.matches(rel):
                continue
            summary = entry.get("summary", {})
            lines = summary.get("lines", {"count": 0, "covered": 0})
            branches = summary.get("branches", {"count": 0, "covered": 0})
            agg = files.setdefault(rel, {"lines": [0, 0], "branches": [0, 0]})
            agg["lines"][0] += lines.get("count", 0)
            agg["lines"][1] += lines.get("covered", 0)
            agg["branches"][0] += branches.get("count", 0)
            agg["branches"][1] += branches.get("covered", 0)

    # Every in-scope translation unit must be present in the report.
    missing_from_report = []
    for cpp in sorted(args.root.rglob("*")):
        if cpp.suffix not in {".cpp", ".cc", ".cxx"}:
            continue
        rel = cpp.relative_to(args.root).as_posix()
        if scope.matches(rel) and rel not in files:
            missing_from_report.append(rel)

    failures = []
    exempted = []
    for rel in sorted(files):
        if rel in exemptions:
            exempted.append(rel)
            continue
        lc, lcv = files[rel]["lines"]
        bc, bcv = files[rel]["branches"]
        problems = []
        if lcv < lc:
            problems.append("lines {}/{}".format(lcv, lc))
        if bcv < bc:
            problems.append("branches {}/{}".format(bcv, bc))
        if problems:
            failures.append((rel, ", ".join(problems)))
    failures.extend((rel, "not linked into any test binary") for rel in missing_from_report)

    total_files = len(files) + len(missing_from_report)
    gate_files = total_files - len(exempted)
    print("=" * 72)
    print("Coverage gate: 100% lines AND 100% branches required for {} files".format(gate_files))
    if exempted:
        print("Exempted files ({}):".format(len(exempted)))
        for rel in exempted:
            print("  {} -- {}".format(rel, exemptions[rel]))
    if failures:
        print("FAIL: {} file(s) below threshold:".format(len(failures)))
        width = max(len(rel) for rel, _ in failures)
        for rel, what in failures:
            print("  {}  {}".format(rel.ljust(width), what))
        print("=" * 72)
        return 1
    print("PASS: all in-scope files at 100% lines and 100% branches.")
    print("=" * 72)

    if args.summary:
        payload = {
            "result": "pass",
            "files": gate_files,
            "exempted": len(exempted),
            "entries": {
                rel: {
                    "lines": {"count": v["lines"][0], "covered": v["lines"][1]},
                    "branches": {"count": v["branches"][0], "covered": v["branches"][1]},
                }
                for rel, v in sorted(files.items())
            },
        }
        args.summary.parent.mkdir(parents=True, exist_ok=True)
        args.summary.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
