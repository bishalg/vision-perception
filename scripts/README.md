# Developer Scripts

All scripts are bash and must stay compatible with the stock macOS bash 3.2
(no `mapfile`; guard empty-array expansion under `set -u`).

| Script | Purpose |
|---|---|
| `setup_dev.sh` | One-time dependency bootstrap (Homebrew / APT) |
| `build.sh` | Configure + parallel build + ctest + CLI smoke runs |
| `test.sh` | Run the full CTest suite |
| `format.sh` | clang-format in place; `--check` mode used by CI. Requires the pinned version: `python3 -m pip install clang-format==18.1.8` |
| `lint.sh` | clang-tidy over first-party sources using `build/compile_commands.json`; `--fix` applies safe auto-fixes. Auto-injects the macOS SDK sysroot; excludes `adapters/android` and `adapters/apple` (NDK/Xcode toolchains) |
| `coverage.sh` | llvm-profdata/llvm-cov pipeline + 100% line+branch coverage gate (`coverage_gate.py`, scope in `coverage_scope.txt`, justified exemptions in `coverage_exemptions.txt`) |

Typical loop:

```bash
./scripts/build.sh && ./scripts/test.sh
./scripts/format.sh && ./scripts/lint.sh
```
