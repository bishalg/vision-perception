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
download/bootstrap OR drop captures
  → pnpm studio  (CurvStudio: draw → Save Truth → Compare)
  → *_USER_CORRECTED*.json  (reviewed:true)
  → scripts/admin_metrics.py matches AI coords vs yours (TP/FP/FN/TN + px error)
  → fix scorer → trends move → repeat
```

Operator how-to (step 1 → step 9): **[`docs/STUDIO_GUIDE.md`](STUDIO_GUIDE.md)**.

- Gallery: filmstrip with `gold` / `bootstrap` / reviewed / `T|V|E` badges + per-line error.
- Annotator: CurvStudio at `http://localhost:8080/apps/annotation_studio/` (`pnpm studio`).
  Legacy `tools/palm_annotator` (port 8765) is **deprecated** — do not start new labeling there.
- Bootstrap: `python3 scripts/download_palm_labels.py --limit 100` (CC BY only; raw JPEGs gitignored).
- Compare: in-studio Draw overlay + Compare mode (Source | Engine | Yours); admin dashboard for batch.
- Fix: retune (`docs/palm_sweep.md`) or retrain; history proves direction.
- **Do not train U-Net until ~50–100 labels have `reviewed:true`.**

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

**Deferred (do not start yet):** heatmap export (`tools/palm_label_export.py`),
MLP linker, and U-Net 4-channel training wait until ~50–100 labels have
`reviewed:true`. See `docs/palm_robust_roadmap.md` Week 3.

## 3. Layout

- Raw drops: `data/samples/mobile_captures/<stem>.jpg` (phone gold) + `data/palm_labels/raw/` (Roboflow bootstrap, gitignored).
- Truth: `<stem>_USER_CORRECTED*.json` next to the capture / under `data/palm_labels/` (`reviewed:true` after annotator Save).
- Schema + howto: [`data/palm_labels/README.md`](../data/palm_labels/README.md).
- Never commit binaries you don't own; manifests reference URLs
  (pattern: `data/fixtures/palm_fate/manifest.json`, `data/palm_labels/manifest.json`).
