# Third-Party Dependencies

This directory holds vendored third-party source files, header-only libraries, and provenance documentation.

## Governance & Hygiene
1. **Permissive Licenses Only**: All code in this directory must comply with the greenlist defined in `docs/license_policy.md` (Apache-2.0, MIT, BSD, Boost, zlib).
2. **Provenance Tracking**: Any vendored dependency must have an entry in `third_party/SOURCE_PROVENANCE.md` and `third_party/license_audit.md`.
3. **No Unaudited Code**: No snippet, header, or submodule may be added without prior architectural review.
