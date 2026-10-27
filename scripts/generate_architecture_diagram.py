#!/usr/bin/env python3
"""Generate the hackathon system-architecture diagram (PNG).

Renders docs/assets/hackathon_architecture.png from fixed layout data so the
diagram stays reproducible. Layout space is SVG-like pixels on a 1600x980
canvas; matplotlib sizes are converted from px to points via PX2PT.
"""

from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import FancyArrowPatch, FancyBboxPatch

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs/assets/hackathon_architecture.png"

W, H, DPI = 1600, 980, 200
PX2PT = 72.0 / DPI  # matplotlib font sizes / linewidths are in points

BLUE_FACE, BLUE_EDGE = "#eff6ff", "#2563eb"
ORANGE_FACE, ORANGE_EDGE = "#fff7ed", "#ea580c"
GREEN_FACE, GREEN_EDGE = "#f0fdf4", "#16a34a"
GRAY_FACE, GRAY_EDGE = "#f8fafc", "#475569"
BAND_FACE, BAND_EDGE = "#f8fafc", "#94a3b8"
TITLE, BODY, MUTED = "#0f172a", "#475569", "#64748b"
INK = "#475569"

plt.rcParams["font.family"] = "DejaVu Sans"


def box(ax, x, y, w, h, face, edge, title, lines=(), title_px=17, lw_px=2.0):
    ax.add_patch(
        FancyBboxPatch(
            (x, y), w, h, boxstyle="round,pad=0,rounding_size=10",
            facecolor=face, edgecolor=edge, linewidth=lw_px * PX2PT,
        )
    )
    if lines:
        ax.text(x + w / 2, y + 24, title, ha="center", va="top",
                fontsize=title_px * PX2PT, fontweight="bold", color=TITLE)
        for i, line in enumerate(lines):
            ax.text(x + 24, y + 52 + i * 21, line, ha="left", va="top",
                    fontsize=12.5 * PX2PT, color=BODY)
    else:
        ax.text(x + w / 2, y + h / 2, title, ha="center", va="center",
                fontsize=title_px * PX2PT, fontweight="bold", color=TITLE)


def arrow(ax, x1, y1, x2, y2, color=INK, dashed=False, both=False):
    ax.add_patch(
        FancyArrowPatch(
            (x1, y1), (x2, y2), arrowstyle="-|>", mutation_scale=8,
            linewidth=1.8 * PX2PT, color=color,
            linestyle=(0, (6, 5)) if dashed else "solid",
            shrinkA=0, shrinkB=0,
        )
    )
    if both:
        ax.add_patch(
            FancyArrowPatch(
                (x1, y1), (x2, y2), arrowstyle="<|-", mutation_scale=8,
                linewidth=1.8 * PX2PT, color=color, linestyle=(0, (6, 5)),
                shrinkA=0, shrinkB=0,
            )
        )


def elbow(ax, pts, color=INK):
    for i in range(len(pts) - 2):
        ax.plot([pts[i][0], pts[i + 1][0]], [pts[i][1], pts[i + 1][1]],
                color=color, linewidth=1.8 * PX2PT, solid_capstyle="butt")
    arrow(ax, pts[-2][0], pts[-2][1], pts[-1][0], pts[-1][1], color=color)


fig, ax = plt.subplots(figsize=(W / DPI, H / DPI), dpi=DPI)
ax.set_xlim(0, W)
ax.set_ylim(H, 0)
ax.axis("off")
fig.subplots_adjust(left=0, right=1, top=1, bottom=0)

ax.text(W / 2, 40, "vision-perception — System Architecture", ha="center",
        va="top", fontsize=30 * PX2PT, fontweight="bold", color=TITLE)
ax.text(W / 2, 74, "Sub-pixel curvilinear perception (OpenCV 5) → autonomous "
        "evidence-gated disaster response (AWS Bedrock)", ha="center", va="top",
        fontsize=16 * PX2PT, color=MUTED)

for cx, label, color in ((260, "PERCEPTION — OPENCV 5 (C++20)", BLUE_EDGE),
                         (740, "AWS EVENTING & STATE", ORANGE_EDGE),
                         (1280, "AGENTIC VISION LOOP — BEDROCK", GREEN_EDGE)):
    ax.text(cx, 112, label, ha="center", va="top", fontsize=15 * PX2PT,
            fontweight="bold", color=color)

# ---- Lane 1: perception -------------------------------------------------
box(ax, 40, 130, 440, 88, BLUE_FACE, BLUE_EDGE, "Sentinel-2 L2A 10 m Imagery",
    ["PRE 2026-08-24 · POST 2026-08-27 · CTRL 2026-08-12",
     "Lende Khola / Bhote Koshi corridor, Nepal"])
arrow(ax, 260, 218, 260, 230)

box(ax, 40, 232, 440, 170, BLUE_FACE, BLUE_EDGE, "C++20 Core Engine · OpenCV 5",
    ["• Steger sub-pixel ridge extraction (Hessian tensors)",
     "• Gaussian scale-space second derivatives",
     "• Sub-pixel ECC co-registration (0.154 px shift)",
     "• Accuracy RMSE 0.0301 px — gate < 0.1 px ✓",
     "• Zero core modifications for cloud deployment"])
arrow(ax, 260, 402, 260, 430)

box(ax, 40, 432, 440, 110, BLUE_FACE, BLUE_EDGE, "Geospatial Flood Domain Pack",
    ["• Cloud mask · channel-width proxy · river classes",
     "• Flood signature +88.4% · width +121.1%",
     "• Dual controls: spatial 0.151× · temporal 0.905× (< 1.10×)"])
arrow(ax, 260, 542, 260, 570)

box(ax, 40, 572, 440, 72, "#dbeafe", BLUE_EDGE, "flood_delta.json",
    ["quantitative perception evidence → uploaded to S3"], title_px=16)

# ---- Lane 2: AWS eventing ----------------------------------------------
box(ax, 560, 572, 360, 64, ORANGE_FACE, ORANGE_EDGE,
    "Amazon S3 — Data Bucket", ["vision-perception-flood-data"], title_px=16)
arrow(ax, 480, 604, 558, 604)
ax.text(518, 594, "upload", ha="center", va="top", fontsize=13 * PX2PT,
        color=MUTED)
arrow(ax, 740, 572, 740, 528)

box(ax, 560, 460, 360, 64, ORANGE_FACE, ORANGE_EDGE, "S3 Event Notification",
    ["PUT flood_delta.json → trigger"], title_px=16)
arrow(ax, 740, 460, 740, 414)

box(ax, 560, 340, 360, 72, ORANGE_FACE, ORANGE_EDGE,
    "AWS Lambda — agent_trigger.py",
    ["event router · invokes Bedrock agent"], title_px=16)
elbow(ax, [(920, 376), (960, 376), (960, 164), (996, 164)])
ax.text(952, 280, "invokes agent", ha="center", va="top",
        fontsize=13 * PX2PT, color=MUTED, rotation=90)

box(ax, 560, 200, 360, 92, ORANGE_FACE, ORANGE_EDGE,
    "Amazon DynamoDB — Event Table",
    ["vision-perception-events-dev",
     "idempotency · evidenceToken · alert state"], title_px=16)
arrow(ax, 740, 340, 740, 294)

arrow(ax, 920, 262, 998, 262, color=GREEN_EDGE, dashed=True, both=True)
ax.text(958, 248, "token · state", ha="center", va="top",
        fontsize=13 * PX2PT, color=GREEN_EDGE)

# ---- Lane 3: agentic loop ----------------------------------------------
box(ax, 1000, 130, 560, 64, GREEN_FACE, GREEN_EDGE,
    "Amazon Bedrock Agent — Claude 3.5 Sonnet v2",
    ["evidence-first system prompt · two-tool action group"], title_px=18)
arrow(ax, 1280, 194, 1280, 222)

box(ax, 1000, 224, 560, 84, GREEN_FACE, GREEN_EDGE,
    "STEP 1 — getFloodEvidence (Perception)",
    ["agent autonomously fetches evidence · issues cryptographic",
     "evidenceToken · registers PENDING event"], title_px=18)
arrow(ax, 1280, 308, 1280, 336)

box(ax, 1000, 338, 560, 84, GREEN_FACE, GREEN_EDGE,
    "STEP 2 — Reasoning (Policy Enforcement)",
    ["strict scientific thresholds: flood_signal_detected ·",
     "width_ratio ≥ 2.0× · controls < 1.10×"], title_px=18)
arrow(ax, 1280, 422, 1280, 450)

box(ax, 1000, 452, 560, 84, GREEN_FACE, GREEN_EDGE,
    "STEP 3 — dispatchFloodAlert (Action)",
    ["validates evidenceToken · enforces DynamoDB idempotency",
     "(eliminates duplicate alerts on retries)"], title_px=18)

arrow(ax, 1280, 536, 1084, 568)
arrow(ax, 1280, 536, 1280, 570)
arrow(ax, 1280, 536, 1476, 568)

box(ax, 1000, 572, 168, 64, GRAY_FACE, GRAY_EDGE, "Amazon SNS",
    ["emergency alert"], title_px=16)
box(ax, 1196, 572, 168, 64, GRAY_FACE, GRAY_EDGE, "Amazon SQS",
    ["fallback queue"], title_px=16)
box(ax, 1392, 572, 168, 64, GRAY_FACE, GRAY_EDGE, "S3 Audit Bucket",
    ["immutable receipt"], title_px=16)

# ---- COOL band ----------------------------------------------------------
ax.add_patch(FancyBboxPatch((40, 680), 1520, 180,
             boxstyle="round,pad=0,rounding_size=12",
             facecolor=BAND_FACE, edgecolor=BAND_EDGE,
             linewidth=2.0 * PX2PT))
ax.text(64, 712, "Best Use of COOL — Cloud-Optimized OpenCV on AWS Graviton "
        "(same zero-modification C++ core)", ha="left", va="top",
        fontsize=19 * PX2PT, fontweight="bold", color=TITLE)

box(ax, 64, 730, 440, 100, BLUE_FACE, BLUE_EDGE,
    "EC2 Graviton3 · c7g.2xlarge (Arm64)",
    ["COOL — Cloud-Optimized OpenCV Library",
     "Neoverse V1 · NEON / SVE · -mcpu=neoverse-v1"], title_px=17)
box(ax, 540, 730, 440, 100, ORANGE_FACE, ORANGE_EDGE,
    "EC2 Intel · c7i.2xlarge (x86_64)",
    ["Standard OpenCV distribution baseline",
     "AVX-512 / AVX2 · -march=native"], title_px=17)
box(ax, 1016, 730, 520, 100, GREEN_FACE, GREEN_EDGE,
    "scripts/benchmark_cool_vs_x86.sh",
    ["100-run latency (P50 / P90 / P95) · cost per scene",
     "SIMD verification via cv2.getBuildInformation()"], title_px=17)

ax.text(64, 852, "Measured reference run (100 iterations, Arm64 "
        "NEON_DOTPROD/FP16 active): P50 213.6 ms · mean 217.4 ms · P95 255.8 ms"
        " · 50,744 scenes / $1.00 — data/benchmark_results.json",
        ha="left", va="top", fontsize=14 * PX2PT, color=BODY)

# ---- Legend -------------------------------------------------------------
for x, face, edge, label in ((40, BLUE_FACE, BLUE_EDGE, "OpenCV 5 / C++ perception"),
                             (260, ORANGE_FACE, ORANGE_EDGE, "AWS services"),
                             (410, GREEN_FACE, GREEN_EDGE, "Agentic loop"),
                             (550, GRAY_FACE, GRAY_EDGE, "Storage / state / alerting")):
    ax.add_patch(FancyBboxPatch((x, 912), 16, 16,
                 boxstyle="round,pad=0,rounding_size=3",
                 facecolor=face, edgecolor=edge, linewidth=1.6 * PX2PT))
    ax.text(x + 24, 920, label, ha="left", va="center",
            fontsize=14 * PX2PT, color=BODY)
ax.text(1560, 920, "OpenCV AI Competition 2026 · vision-perception",
        ha="right", va="center", fontsize=14 * PX2PT, color="#94a3b8")

OUT.parent.mkdir(parents=True, exist_ok=True)
fig.savefig(OUT, dpi=DPI, facecolor="white")
print(f"saved {OUT}")
