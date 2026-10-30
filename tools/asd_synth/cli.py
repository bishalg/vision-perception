"""CLI: python3 -m tools.asd_synth.cli --count 50 --seed 7 --out ./synthetic_sessions"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .generator import generate_session


def main(argv=None) -> int:
    p = argparse.ArgumentParser(description="Write N synthetic ASD gaze sessions (session-v1.6).")
    p.add_argument("--count", "-n", type=int, default=20)
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("--out", type=Path, required=True, help="output directory (created)")
    a = p.parse_args(argv)
    if a.count < 0:
        p.error("--count must be >= 0")
    a.out.mkdir(parents=True, exist_ok=True)
    for i in range(a.count):
        s = generate_session(a.seed, i)
        (a.out / f"{s['sessionId']}.json").write_text(
            json.dumps(s, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    print(f"Wrote {a.count} SYNTHETIC sessions to {a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
