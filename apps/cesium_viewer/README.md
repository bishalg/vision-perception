# Phase 4 — CesiumJS CCTV Digital Twin

Standalone Vite + CesiumJS app (no NX). Renders fixture cameras as globe markers with
pose metadata and still-image panels. **No Cesium ion token required** (OSM tiles + ellipsoid).

> End goal is the **alert + forecast**, not the lines: analysts review detections
> on the twin, confirm or correct them, and confirmations improve thresholds.
> Principle: [`docs/product_vision.md`](../../docs/product_vision.md).

## Quick start

```bash
# 1. Export GeoJSON + copy PNG stills into public/media
python3 tools/geojson_exporter.py

# 2. Install & run
cd apps/cesium_viewer
npm install
npm run dev
# → http://localhost:3000
```

## Demo script (~2.5 min)

1. Show architecture (C++ fixtures → GeoJSON → Cesium).
2. Run `npm run dev` and fly to I-80 Drum Forebay.
3. Click a camera → panel shows agency, pose, fixture still.
4. Point at Bhote Koshi pier for the Nepal flood corridor.

## Regenerating data

```bash
python3 tools/geojson_exporter.py \
  -o apps/cesium_viewer/public/cameras.geojson \
  --media-dir apps/cesium_viewer/public/media
```
