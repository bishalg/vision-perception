# Third-Party Source Code Provenance Register

This document registers the origin, license pedigree, and modification history of any external source code incorporated into `vision-perception`.

---

## 1. Register Template

Every new dependency must add an entry matching this template before merging:

```markdown
### Package: <Name>
- **Upstream Repository**: <URL>
- **Commit SHA / Release Tag**: <Hash or Tag>
- **Original Author / Copyright**: <Holder>
- **License**: <License Identifier (SPDX)>
- **Integration Target**: `third_party/<directory>/`
- **Modifications**:
  - [ ] None (unmodified snapshot)
  - [ ] Custom modifications (detailed below)
- **Review Date**: YYYY-MM-DD
- **Approved By**: <Name>
```

---

## 2. Active Entries

*(No external source files have been copied or vendored into the repository at Milestone 1 scaffold stage).*
