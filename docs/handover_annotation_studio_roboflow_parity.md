# Handover: CurvStudio — Roboflow-Grade Annotation & Evaluation Platform

> **Date**: 2026-09-11  
> **Audience**: Next AI Agent / Engineer continuing development of the unified vision platform  
> **Objective**: Deliver a robust, zero-cost ($0/mo), local, privacy-first computer vision studio matching **Roboflow's UI/UX and feature caliber** for both **Palm Creases** and **Flood / River Morphology**.

---

## 1. Executive Summary & Vision

We are building **CurvStudio** as an integrated, first-class feature of the `vision-perception` monorepo. Instead of relying on expensive cloud subscriptions ($100+/month) or external SaaS tools that compromise proprietary phone captures and satellite imagery, `CurvStudio` gives us:
1. **High-Precision Multi-Domain Annotation**: Polygons, polylines, brush strokes, and AI-assisted contour snapping.
2. **Dataset Curation & Partitioning**: Interactive 3-way dataset splitting (**Train / Valid / Test**: 70% / 20% / 10%) directly synced with dataset manifests.
3. **Live Model Inference & Test Bench**: Interactive evaluation runner with confidence threshold sliders, layer overlays, and instant metric recalculation.
4. **Zero Cloud Dependencies**: 100% local execution served directly from `pnpm studio` or `pnpm dev`.

---

## 2. Visual Feature Breakdown (Analyzed from Roboflow UI)

The user provided 7 direct UI/UX reference screenshots from Roboflow. The next agent should use these exact UX paradigms:

### A. Floating Right Tool Dock
- **Shape**: Vertical floating pill on the right edge with subtle glassmorphism (`backdrop-filter: blur(12px)`).
- **Core Tools**:
  - `✋ Pan / Hand (H)`: Spacebar or drag to pan; mouse wheel to zoom centered on cursor.
  - `⬡ Polygon Tool (P)`: Freeform click-to-place vertices, drag handles to edit, double-click or `Enter` to commit.
  - `✏️ Polyline / Line Tool (L)`: Essential for palm creases and flood waterlines.
  - `🖌️ Brush Tool (B)`: Stroke painting with floating sub-popover for **Mode (Draw/Erase)** and **Size slider**.
  - `▢ Bounding Box (B)`: Rectangular box annotation.
  - `AI Assist Section` (marked with `AI` badge):
    - Magic Wand / Contour Snap: Click near any CurvEngine Steger ridge to adopt it.
  - `Undo (Cmd+Z)` / `Redo (Cmd+Y)`.
  - `Mark Empty / Absent (A)`: Flag that the active line or feature does not exist on this capture.
- **Hover Preview Cards**: Hovering over tool icons displays rich explanation cards with shortcut keys and mini visual diagrams.

### B. In-Context Annotation Popover
- Floating card positioned directly over the active shape/cursor when a polyline or polygon is finished:
  - Text input for class search / label creation.
  - Quick-select buttons with color indicators and numeric hotkeys (`1`, `2`, `3`, `4`).
  - Action buttons: `Delete` and `Save (Enter)`.

### C. Left Sidebar (Multi-Tab Navigation)
- **Tab 1: Labels / Classes**:
  - Active annotation counts (e.g. `Annotations: 4`).
  - Class palette with color dots, label names, and visibility toggles (eye icon).
  - Metadata tag pills (e.g. `phone-gold`, `sentinel2-l2a`, `verified`).
- **Tab 2: Layers**:
  - Independent opacity sliders (`0%` to `100%`) for:
    1. Background Source Image
    2. CurvEngine AI Predictions / Steger Ridges
    3. Human Ground Truth Annotations
- **Tab 3: Raw Data (JSON Inspector)**:
  - Syntax-highlighted live JSON viewer showing `Source Data` and `Annotation Data`.
  - 1-click `Copy` button.
- **Tab 4: History / Revisions**:
  - Audit trail of edits and review timestamps.

### D. Dataset Management & Train / Valid / Test Splitter
- Top bar button: **"Add to Dataset"** opens a dedicated modal:
  - "Labeled Images to Add: X", "Remaining Batch Images: Y".
  - **3-Way Range Slider**: `70% Train` (purple), `20% Valid` (blue), `10% Test` (orange).
  - Automatically updates image counts in real time.
  - Action button commits the partitioning into `manifest.json`.

### E. Live Model Test Bench (Inference Playground)
- Dedicated mode to test models on new images:
  - Source selector: `File Upload`, `Asset Library`, `Webcam / Stream`.
  - Confidence threshold slider (`0%` to `100%`).
  - Visual output view showing overlay masks, bounding boxes, or creases with confidence percentages (e.g. `water 64%`, `fate_line 76%`).
  - `Rerun` button to re-execute the C++ / ONNX pipeline on the fly.

---

## 3. What Has Already Been Implemented (Current Foundations)

Do **not** start from scratch. The following core pieces are already built, tested, and passing all gates:

| Component | Path | Status |
| :--- | :--- | :--- |
| **Domain Registry** | [`configs/domain_registry.json`](../configs/domain_registry.json) | Configures Palm (`heart`, `head`, `life`, `fate`) and Flood (`waterline`, `flood_boundary`, `river_centerline`, `bank_line`) |
| **Studio Web App** | [`apps/annotation_studio/index.html`](../apps/annotation_studio/index.html) | Canvas engine with Pan/Zoom, Polyline draw, AI snap, layer opacity, and bottom gallery |
| **Studio Local Server** | [`scripts/studio_server.py`](../scripts/studio_server.py) | Python standard library server (`GET /api/domains`, `GET /api/samples`, `POST /api/save_truth`) |
| **Desktop C++ Debugger** | [`tools/lib/curv/tools/palm_overlay_debug_lib.cpp`](../tools/lib/curv/tools/palm_overlay_debug_lib.cpp) | Native OpenCV viewer with on-screen HUD, clickable buttons, and sliders (zero terminal blocking) |
| **Admin Metrics Engine** | [`scripts/admin_metrics.py`](../scripts/admin_metrics.py) | Calculates TP/FP/FN/TN, Hausdorff px error, and length ratio; scans `mobile_captures` & `palm_labels` |
| **Monorepo Menu** | [`scripts/menu/`](../scripts/menu/) & `package.json` | `pnpm studio` and `pnpm dev` with `[STUDIO]`, `[DESKTOP GUI]`, `[CLI]` tags |
| **C++ Test Suite** | `ctest --test-dir build` | **100% passing (32/32 tests)** |

---

## 4. File Map for Next Agent

```text
vision-perception/
├── configs/
│   ├── domain_registry.json                 # Multi-domain schema (Palm & Flood classes/paths)
│   └── geospatial_wide_ridge.json           # Steger parameters for flood Sentinel-2
├── apps/
│   └── annotation_studio/
│       └── index.html                       # Core studio interface (to be upgraded with Roboflow UX)
├── scripts/
│   ├── studio_server.py                     # Local REST API & static server
│   ├── admin_metrics.py                     # Metric calculation engine & HTML dashboard
│   ├── download_palm_labels.py              # Roboflow CC-BY bootstrap downloader
│   └── menu/
│       ├── labeling.mjs                     # Monorepo labeling menu items
│       └── flood.mjs                        # Monorepo flood menu items
├── data/
│   ├── samples/mobile_captures/             # Phone gold captures (01_left, 02_right)
│   ├── flood_nepal_2026/                    # Sentinel-2 flood acquisition images & evidence
│   └── palm_labels/                         # Bootstrap labels + manifest.json
└── tools/
    └── palm_annotator/                      # Legacy palm-specific annotator (precursor to CurvStudio)
```

---

## 5. Step-by-Step Progressive Roadmap for the Next Agent

The next engineer or AI agent should execute these steps in order:

### Step 1: Roboflow Floating Tool Dock & Hover Previews
- In [`apps/annotation_studio/index.html`](../apps/annotation_studio/index.html):
  - Replace the left tool buttons with a **vertical floating pill dock on the right**.
  - Add tool icons: Hand (`H`), Polygon (`P`), Polyline (`L`), Brush (`B`), AI Wand (`S`), Undo, Redo, Empty (`A`).
  - Add floating hover tooltip cards explaining each tool with shortcut key badges.

### Step 2: In-Context Annotation Popover
- When a user finishes drawing a polyline or polygon on the canvas:
  - Display a floating glass popover adjacent to the completed shape.
  - Provide an input field with quick class chips (`1: heart`, `2: head`, `3: life`, `4: fate` or `1: waterline`, `2: flood_boundary`).
  - Pressing `Enter` commits the line to that class; pressing `Escape` or `Delete` discards it.

### Step 3: Raw Data Inspector Tab
- In the left sidebar, add the `Raw Data` tab (matching Roboflow screenshot #4):
  - Display the active fixture's source metadata and current annotation JSON.
  - Implement 1-click `Copy JSON` button.

### Step 4: Train / Valid / Test Splitter Modal
- Add the **"Add to Dataset"** button on the top right:
  - Clicking opens the Roboflow-style modal with the 3-way split range slider (70% Train, 20% Valid, 10% Test).
  - In [`scripts/studio_server.py`](../scripts/studio_server.py), add endpoint `POST /api/dataset_split`:
    - Persists split tags (`train`, `valid`, `test`) to `manifest.json` in the respective dataset folder.

### Step 5: Live Test Bench / Model Inference Playground
- Add a "Test Model" tab/view (matching Roboflow screenshots #6 & #7):
  - Allows selecting images from the Asset Library or uploading a new file.
  - Confidence threshold slider (`0%` to `100%`).
  - Triggers CurvEngine C++ pipeline (`hand_inspector_cli` or `curv_cli`) on the backend via a new endpoint `POST /api/run_inference`.
  - Overlays predicted segmentation masks and polylines with confidence percentage badges.

---

## 6. Strict Development Guardrails

1. **Zero Cloud API Costs**:
   - Everything must run locally. Do not integrate external paid SaaS APIs. The local C++ CurvEngine backend and Python server handle all data and inference.
2. **Modular File Limits**:
   - Keep files under 800 lines. If `index.html` or `studio_server.py` grows, extract modular JS/CSS files (e.g. `apps/annotation_studio/js/canvas.js`, `tools.js`).
3. **Privacy & Data Security**:
   - Never commit raw downloaded images or sensitive phone captures to git. Verify `.gitignore` covers `data/palm_labels/raw/`.
4. **License Integrity**:
   - Only CC BY 4.0 images are allowed into the training tree. Refuse CC-BY-NC (non-commercial) datasets.
5. **No ML Training Yet**:
   - Do **not** train U-Net or deep learning models until at least **50–100** high-quality annotations have `reviewed: true`. Bad labels degrade model weights.
6. **Code Quality**:
   - Ensure `ctest --test-dir build` continues passing with 100% success (32/32 tests).
   - Run `./scripts/format.sh` after modifying any C++ code.

---

## 7. How to Launch and Verify

To spin up the studio at any time:
```bash
# Start studio server and open in browser:
pnpm studio

# Or via monorepo menu:
pnpm dev
# Select: 4. [LABELING] -> 1. [STUDIO] Launch CurvStudio
```
To run backend metric checks:
```bash
python3 scripts/admin_metrics.py --markdown
```
To run full test suite:
```bash
ctest --test-dir build --output-on-failure
```
