# Training Data Strategy — Gallery → Label → Compare → Fix

> Verdict: yes, this loop is the best way forward. Your own 100s of phone
> hands are the gold set (same camera, same warp, our landmarks); free
> internet data bootstraps robustness around it. Literature agrees: the
> U-Net palm-line paper (arXiv 2102.12127) handcrafted its dataset from
> scratch, and fate-line ML work leans on transfer learning exactly because
> labeled fate data is scarce.

---

## 1. The loop (already running)

```text
drop captures → gallery shows array (open output/admin_dashboard.html)
  → you draw each major line → *_USER_CORRECTED*.json
  → scripts/admin_metrics.py matches AI coords vs yours (TP/FP/FN/TN + px error)
  → fix scorer → trends move → repeat
```

- Gallery: image array with `reviewed` / `needs label` badges + per-line error.
- Compare: presence confusion matrix + mean point error per line.
- Fix: retune (`docs/palm_sweep.md`) or retrain; history proves direction.

## 2. Free internet sources (suitability-ranked)

| Source | What you get | License / access | Use for us |
| :--- | :--- | :--- | :--- |
| Roboflow Universe: palm-lines recognition (~3k imgs, keypoints `heart/head/life/fate`) | Direct crease labels | CC BY 4.0 (cite) | Bootstrap line detector; **review label quality first** (community-made) |
| Roboflow: palmistry YOLO set (~6.7k, `life/heart/head/fate` boxes) | Line bounding regions | CC BY 4.0 (cite); see `zohaakhawajaa/palm-line-detection-yolov8` | Weak labels / pre-filter |
| Roboflow: palm-lines instance-seg (50 imgs) | Pixel masks, 6 classes | CC BY 4.0 | Tiny — eval only |
| 11k Hands (11,076 imgs, 190 subjects, palmar+dorsal, 1600×1200) | Full hands, no crease labels | Free academic fair use ([sites.google.com/view/11khands](https://sites.google.com/view/11khands), Kaggle mirror) | Outline/pose robustness, hard negatives; not crease truth |
| Biometric DBs: CASIA / PolyU / Tongji / IITD / NTU-CP-v1 / XJTU-UP (mobile) | Palmprint ROIs + texture | Varies; NTU needs signed Data Release Agreement | Texture/backbone pretraining; cropped ROIs ≠ our full-hand phone domain |
| FreiHAND / HaGRID | 3D pose / gesture boxes | Research licenses | Landmark robustness only; no creases |

Rule: internet data **pretrains and stress-tests**; your phone captures
**decide**. Anything downloaded gets logged in
`third_party/SOURCE_PROVENANCE.md` with license + URL before use.

## 3. Layout

- Raw drops: `data/samples/mobile_captures/<stem>.jpg` (today) → `data/palm_labels/` at scale (see `docs/palm_robust_roadmap.md` D1–D2).
- Truth: `<stem>_USER_CORRECTED*.json` (+`.png`) next to the capture.
- Never commit binaries you don't own; manifests reference URLs
  (pattern: `data/fixtures/palm_fate/manifest.json`).
