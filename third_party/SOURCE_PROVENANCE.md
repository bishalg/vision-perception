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

### Dataset: Roboflow palms-lf79v/palm-lines-recognition-lbedg
- **Upstream Repository**: https://universe.roboflow.com/palms-lf79v/palm-lines-recognition-lbedg
- **Commit SHA / Release Tag**: version 1
- **Original Author / Copyright**: Roboflow Universe project `palms-lf79v/palm-lines-recognition-lbedg`
- **License**: CC BY 4.0 (CC BY required; verified by download script)
- **Integration Target**: `data/palm_labels/` (raw images gitignored; JSON labels + manifest committed)
- **Modifications**:
  - [x] Keypoints converted to `palm_label_v1` polylines (`reviewed: false` until human Save)
- **Sample count (this machine)**: 50
- **Review Date**: 2026-09-11
- **Approved By**: download_palm_labels.py (automated)


### Dataset: Roboflow unitec-gr8dy/river-flood-detection
- **Upstream Repository**: https://universe.roboflow.com/unitec-gr8dy/river-flood-detection
- **Commit SHA / Release Tag**: version 6
- **Original Author / Copyright**: Roboflow Universe project `unitec-gr8dy/river-flood-detection`
- **License**: CC BY 4.0 (CC BY required; verified by download script)
- **Integration Target**: `data/flood_labels/` (raw images gitignored; JSON labels + manifest committed)
- **Modifications**:
  - [x] COCO segmentation polygons converted to `curv_v1` polylines (`reviewed: false` until human Save)
- **Sample count (this machine)**: 50
- **Review Date**: 2026-09-11
- **Approved By**: download_flood_labels.py (automated)
