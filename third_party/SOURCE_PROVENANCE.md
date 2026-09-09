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

### Package: OpenCV (iOS XCFramework)
- **Upstream Repository**: https://github.com/opencv/opencv
- **Commit SHA / Release Tag**: `4.11.0`
- **Original Author / Copyright**: OpenCV team / OpenCV Foundation
- **License**: Apache-2.0
- **Integration Target**: `third_party/ios/` (built on demand, git-ignored — see
  `scripts/build_opencv_ios.sh`; not committed)
- **Modifications**:
  - [x] None (unmodified upstream, compiled for `iphoneos`/`iphonesimulator` arm64 with
    `videoio`/`highgui`/`gapi`/language-binding modules disabled; `core`/`imgproc`/`imgcodecs`/`dnn` kept)
- **Review Date**: 2026-09-09
- **Approved By**: Bishal Ghimire
