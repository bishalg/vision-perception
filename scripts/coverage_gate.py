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

Exemptions (scripts/coverage_exemptions.txt), '|' separated:
    <repo-relative-path>|<justification>                     -> whole file exempt
    <repo-relative-path>|<function-name-substring>|<justification>
                                                             -> function exempt
    Function-level exemptions subtract only the untaken branch edges and
    unexecuted lines attributable to the matching functions, so exempting one
    defensive helper never ungates the rest of its file. Exemptions exist for
    provably unreachable defensive code only; every entry carries a written
    justification and is surfaced in the gate output.

Semantics:
    - line coverage:    summary.lines.covered == summary.lines.count
    - branch coverage:  summary.branches.covered == summary.branches.count
      (llvm-cov counts a branch edge covered only when that edge is executed;
      a conditional is at 100% only when BOTH edges are taken)
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


def parse_exemptions(rules):
    """Return (whole_file, per_function) exemption dicts."""
    whole = {}
    per_function = {}
    for entry in rules:
        parts = [p.strip() for p in entry.split("|")]
        if len(parts) == 2:
            whole[parts[0]] = parts[1]
        elif len(parts) == 3:
            per_function.setdefault(parts[0], []).append((parts[1], parts[2]))
        else:
            print("coverage-gate: FATAL malformed exemption entry: {}".format(entry))
            sys.exit(2)
    return whole, per_function


def branch_edges(entry):
    """Untaken edge count of one llvm-cov branch record.

    Record: [line_start, col_start, line_end, col_end, true_count, false_count, ...]
    """
    true_count = entry[4] if len(entry) > 4 else 0
    false_count = entry[5] if len(entry) > 5 else 0
    return (1 if true_count == 0 else 0) + (1 if false_count == 0 else 0)


def function_uncovered(func_record):
    """(uncovered_lines, untaken_branch_edges) attributable to one function.

    Lines: union of zero-count region spans minus every line covered by any
    positive-count region of the same file. This can only UNDERCOUNT the
    subtraction (regions are coarser than segments), keeping the gate strict.
    """
    zero_lines = set()
    covered_lines = set()
    for region in func_record.get("regions", []):
        span = set(range(region[0], region[2] + 1))
        if region[4] == 0:
            zero_lines |= span
        else:
            covered_lines |= span
    untaken = sum(branch_edges(b) for b in func_record.get("branches", []))
    return len(zero_lines - covered_lines), untaken


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

    whole_file_exemptions, function_exemptions = parse_exemptions(load_rule_file(args.exemptions))

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

    # Collect per-function records for in-scope files (function-level exemptions).
    functions_by_file = {}
    for datum in data.get("data", []):
        for func in datum.get("functions", []):
            for filename in func.get("filenames", []):
                rel = rel_path(filename, args.root)
                if rel in files:
                    functions_by_file.setdefault(rel, []).append(func)

    # Every in-scope translation unit must be present in the report.
    missing_from_report = []
    for cpp in sorted(args.root.rglob("*")):
        if cpp.suffix not in {".cpp", ".cc", ".cxx"}:
            continue
        rel = cpp.relative_to(args.root).as_posix()
        if scope.matches(rel) and rel not in files:
            missing_from_report.append(rel)

    failures = []
    exempted_files = []
    function_exemptions_applied = []
    for rel in sorted(files):
        if rel in whole_file_exemptions:
            exempted_files.append(rel)
            continue
        lc, lcv = files[rel]["lines"]
        bc, bcv = files[rel]["branches"]
        uncovered_lines = lc - lcv
        uncovered_branches = bc - bcv

        for pattern, justification in function_exemptions.get(rel, []):
            matched = False
            for func in functions_by_file.get(rel, []):
                if pattern not in func.get("name", ""):
                    continue
                matched = True
                f_lines, f_branches = function_uncovered(func)
                uncovered_lines -= f_lines
                uncovered_branches -= f_branches
            if matched:
                function_exemptions_applied.append((rel, pattern, justification))

        problems = []
        if uncovered_lines > 0:
            problems.append("lines {}/{}".format(lc - uncovered_lines, lc))
        if uncovered_branches > 0:
            problems.append("branches {}/{}".format(bc - uncovered_branches, bc))
        if problems:
            failures.append((rel, ", ".join(problems)))
    failures.extend((rel, "not linked into any test binary") for rel in missing_from_report)

    total_files = len(files) + len(missing_from_report)
    gate_files = total_files - len(exempted_files)
    print("=" * 72)
    print("Coverage gate: 100% lines AND 100% branches required for {} files".format(gate_files))
    if exempted_files:
        print("Exempted files ({}):".format(len(exempted_files)))
        for rel in exempted_files:
            print("  {} -- {}".format(rel, whole_file_exemptions[rel]))
    if function_exemptions_applied:
        print("Function-level exemptions applied ({}):".format(len(function_exemptions_applied)))
        for rel, pattern, justification in function_exemptions_applied:
            print("  {} :: {} -- {}".format(rel, pattern, justification))
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
            "exempted_files": len(exempted_files),
            "function_exemptions": len(function_exemptions_applied),
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
