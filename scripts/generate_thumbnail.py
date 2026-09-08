#!/usr/bin/env python3
"""
scripts/generate_thumbnail.py

Generates the official 3:2 ratio (1800x1200) thumbnail image for the
Devpost OpenCV AI Competition 2026 submission:
  - Project Title: vision-perception: Sub-Pixel AI Flood Early Warning
  - High-contrast PRE vs POST side-by-side with sub-pixel Steger ridge vectors
  - Quantitative scientific metrics HUD (+121% widening, +88.4% surge, 0.0301 px RMSE)
  - Target award badges: Best Use of COOL & Agentic Vision
"""

import cv2
import numpy as np
import os
import sys

PRE_PATH = "data/flood_nepal_2026/out/pre_overlay.png"
POST_PATH = "data/flood_nepal_2026/out/post_overlay.png"
OUT_PNG = "docs/assets/devpost_thumbnail.png"
OUT_JPG = "docs/assets/devpost_thumbnail.jpg"

def draw_glass_card(canvas, x1, y1, x2, y2, bg_color=(20, 24, 32), alpha=0.82, border_color=(80, 100, 130)):
    """Draws a semi-transparent glass card with a subtle border."""
    overlay = canvas.copy()
    cv2.rectangle(overlay, (x1, y1), (x2, y2), bg_color, -1)
    cv2.addWeighted(overlay, alpha, canvas, 1.0 - alpha, 0, canvas)
    cv2.rectangle(canvas, (x1, y1), (x2, y2), border_color, 1, cv2.LINE_AA)

def main():
    if not os.path.exists(PRE_PATH) or not os.path.exists(POST_PATH):
        print(f"Error: Missing input overlays ({PRE_PATH} or {POST_PATH})")
        sys.exit(1)

    pre_img = cv2.imread(PRE_PATH)
    post_img = cv2.imread(POST_PATH)

    # 3:2 aspect ratio target
    WIDTH, HEIGHT = 1800, 1200
    canvas = np.zeros((HEIGHT, WIDTH, 3), dtype=np.uint8)
    canvas[:] = (12, 14, 20)  # Sleek dark background

    # Layout dimensions
    HEADER_H = 140
    FOOTER_H = 110
    BODY_H = HEIGHT - HEADER_H - FOOTER_H  # 950px
    PANEL_W = 880  # 880 * 2 = 1760 + 20 margin each side + 0 center gap = 1800
    GAP = 16
    PANEL_W = (WIDTH - 3 * GAP) // 2  # (1800 - 48) // 2 = 876

    # Crop and resize pre & post to fit PANEL_W x BODY_H
    # Original is 893w x 1172h
    # Focus on the active river canyon (middle vertical region)
    orig_h, orig_w = pre_img.shape[:2]
    crop_h = int(orig_w * (BODY_H / PANEL_W))
    if crop_h > orig_h:
        crop_h = orig_h
        crop_w = int(orig_h * (PANEL_W / BODY_H))
        start_x = (orig_w - crop_w) // 2
        pre_crop = pre_img[:, start_x:start_x+crop_w]
        post_crop = post_img[:, start_x:start_x+crop_w]
    else:
        # River gorge is vertically prominent in the middle
        start_y = int((orig_h - crop_h) * 0.45)
        pre_crop = pre_img[start_y:start_y+crop_h, :]
        post_crop = post_img[start_y:start_y+crop_h, :]

    pre_resized = cv2.resize(pre_crop, (PANEL_W, BODY_H), interpolation=cv2.INTER_AREA)
    post_resized = cv2.resize(post_crop, (PANEL_W, BODY_H), interpolation=cv2.INTER_AREA)

    # Place panels
    x_pre = GAP
    x_post = GAP * 2 + PANEL_W
    y_body = HEADER_H

    canvas[y_body:y_body+BODY_H, x_pre:x_pre+PANEL_W] = pre_resized
    canvas[y_body:y_body+BODY_H, x_post:x_post+PANEL_W] = post_resized

    # Border around imagery panels
    cv2.rectangle(canvas, (x_pre, y_body), (x_pre+PANEL_W, y_body+BODY_H), (50, 70, 95), 2, cv2.LINE_AA)
    cv2.rectangle(canvas, (x_post, y_body), (x_post+PANEL_W, y_body+BODY_H), (0, 165, 255), 2, cv2.LINE_AA)

    # --- 1. HEADER SECTION ---
    # Gradient accent line at top
    cv2.rectangle(canvas, (0, 0), (WIDTH, 4), (0, 210, 255), -1)

    # Category Pill
    cv2.rectangle(canvas, (GAP, 18), (GAP + 340, 48), (35, 45, 60), -1)
    cv2.rectangle(canvas, (GAP, 18), (GAP + 340, 48), (0, 200, 255), 1, cv2.LINE_AA)
    cv2.putText(canvas, "OPENCV AI COMPETITION 2026", (GAP + 15, 39),
                cv2.FONT_HERSHEY_DUPLEX, 0.60, (0, 230, 255), 1, cv2.LINE_AA)

    # Award targets pill (right aligned)
    cv2.rectangle(canvas, (WIDTH - GAP - 480, 18), (WIDTH - GAP, 48), (40, 30, 60), -1)
    cv2.rectangle(canvas, (WIDTH - GAP - 480, 18), (WIDTH - GAP, 48), (180, 100, 255), 1, cv2.LINE_AA)
    cv2.putText(canvas, "BEST USE OF COOL  *  AGENTIC VISION", (WIDTH - GAP - 465, 39),
                cv2.FONT_HERSHEY_DUPLEX, 0.58, (220, 170, 255), 1, cv2.LINE_AA)

    # Main Project Title
    cv2.putText(canvas, "vision-perception : Sub-Pixel AI Flood Early Warning",
                (GAP, 92), cv2.FONT_HERSHEY_DUPLEX, 1.22, (255, 255, 255), 2, cv2.LINE_AA)

    # Subtitle Context
    cv2.putText(canvas, "Nepal Lende Khola / Bhote Koshi Glacier Debris Flood  *  Sentinel-2 Sub-Pixel Curvilinear Tracking",
                (GAP, 124), cv2.FONT_HERSHEY_SIMPLEX, 0.65, (170, 185, 200), 1, cv2.LINE_AA)

    # --- 2. PANEL HEADERS & METRIC CARDS ---
    # Left Panel (PRE-EVENT)
    draw_glass_card(canvas, x_pre + 15, y_body + 15, x_pre + 380, y_body + 85, (15, 20, 28), 0.85, (70, 90, 110))
    cv2.putText(canvas, "PRE-EVENT : 2026-08-24", (x_pre + 30, y_body + 45),
                cv2.FONT_HERSHEY_DUPLEX, 0.72, (220, 220, 220), 1, cv2.LINE_AA)
    cv2.putText(canvas, "Normal Baseflow  *  Width: 19.0 px (190 m)", (x_pre + 30, y_body + 72),
                cv2.FONT_HERSHEY_SIMPLEX, 0.58, (140, 170, 190), 1, cv2.LINE_AA)

    # Right Panel (POST-EVENT)
    draw_glass_card(canvas, x_post + 15, y_body + 15, x_post + 430, y_body + 85, (25, 18, 15), 0.88, (0, 120, 255))
    cv2.putText(canvas, "POST-EVENT : 2026-08-27", (x_post + 30, y_body + 45),
                cv2.FONT_HERSHEY_DUPLEX, 0.72, (0, 165, 255), 2, cv2.LINE_AA)
    cv2.putText(canvas, "DISASTER SURGE  *  Width: 42.0 px (420 m)", (x_post + 30, y_body + 72),
                cv2.FONT_HERSHEY_SIMPLEX, 0.58, (255, 180, 130), 1, cv2.LINE_AA)

    # Floating Big Stats Badge (Bottom of each panel)
    # Left Badge
    draw_glass_card(canvas, x_pre + 15, y_body + BODY_H - 110, x_pre + PANEL_W - 15, y_body + BODY_H - 15,
                    (12, 16, 22), 0.88, (50, 75, 105))
    cv2.putText(canvas, "SUB-PIXEL CORE REGISTRATION", (x_pre + 30, y_body + BODY_H - 78),
                cv2.FONT_HERSHEY_SIMPLEX, 0.52, (130, 160, 180), 1, cv2.LINE_AA)
    cv2.putText(canvas, "ECC Shift: 0.154 px  (Passes < 3.0 px)", (x_pre + 30, y_body + BODY_H - 42),
                cv2.FONT_HERSHEY_DUPLEX, 0.75, (0, 230, 255), 2, cv2.LINE_AA)
    cv2.putText(canvas, "Temporal Baseline: 0.905x", (x_pre + 520, y_body + BODY_H - 42),
                cv2.FONT_HERSHEY_DUPLEX, 0.68, (120, 240, 160), 1, cv2.LINE_AA)

    # Right Badge (Alarm Highlights)
    draw_glass_card(canvas, x_post + 15, y_body + BODY_H - 110, x_post + PANEL_W - 15, y_body + BODY_H - 15,
                    (30, 15, 15), 0.92, (0, 80, 255))
    cv2.putText(canvas, "QUANTITATIVE FLOOD DELTA", (x_post + 30, y_body + BODY_H - 78),
                cv2.FONT_HERSHEY_SIMPLEX, 0.52, (255, 140, 100), 1, cv2.LINE_AA)
    cv2.putText(canvas, "CHANNEL: +121.1%  |  SURGE: +88.4%", (x_post + 30, y_body + BODY_H - 42),
                cv2.FONT_HERSHEY_DUPLEX, 0.85, (0, 90, 255), 2, cv2.LINE_AA)
    cv2.putText(canvas, "Spatial Control: 0.151x", (x_post + 540, y_body + BODY_H - 42),
                cv2.FONT_HERSHEY_DUPLEX, 0.68, (120, 240, 160), 1, cv2.LINE_AA)

    # --- 3. FOOTER TECHNOLOGY BAR ---
    y_foot = HEIGHT - FOOTER_H
    cv2.line(canvas, (GAP, y_foot), (WIDTH - GAP, y_foot), (40, 50, 65), 1)

    cards = [
        ("CORE CV ENGINE", "OpenCV 5 * C++20 * Steger 0.0301 px RMSE", (0, 210, 255)),
        ("AGENTIC VISION", "AWS Bedrock * Claude 3.5 * DynamoDB Idempotency", (220, 140, 255)),
        ("COOL ACCELERATION", "AWS Graviton3 * Neoverse V1 * -25.4% Latency", (0, 200, 120)),
    ]

    card_w = (WIDTH - 4 * GAP) // 3
    for i, (title, desc, color) in enumerate(cards):
        cx1 = GAP + i * (card_w + GAP)
        cx2 = cx1 + card_w
        cy1 = y_foot + 14
        cy2 = HEIGHT - 16
        draw_glass_card(canvas, cx1, cy1, cx2, cy2, (18, 22, 30), 0.85, (50, 65, 85))
        # Left colored pill bar
        cv2.rectangle(canvas, (cx1 + 2, cy1 + 2), (cx1 + 8, cy2 - 2), color, -1)
        cv2.putText(canvas, title, (cx1 + 22, cy1 + 32),
                    cv2.FONT_HERSHEY_DUPLEX, 0.65, color, 1, cv2.LINE_AA)
        cv2.putText(canvas, desc, (cx1 + 22, cy1 + 62),
                    cv2.FONT_HERSHEY_SIMPLEX, 0.48, (180, 195, 210), 1, cv2.LINE_AA)

    # Save PNG and high-quality JPG
    cv2.imwrite(OUT_PNG, canvas, [cv2.IMWRITE_PNG_COMPRESSION, 4])
    cv2.imwrite(OUT_JPG, canvas, [cv2.IMWRITE_JPEG_QUALITY, 95])

    png_size_mb = os.path.getsize(OUT_PNG) / (1024 * 1024)
    jpg_size_mb = os.path.getsize(OUT_JPG) / (1024 * 1024)
    print(f"Generated Devpost thumbnail:")
    print(f"  PNG: {OUT_PNG} ({WIDTH}x{HEIGHT}, {png_size_mb:.2f} MB)")
    print(f"  JPG: {OUT_JPG} ({WIDTH}x{HEIGHT}, {jpg_size_mb:.2f} MB)")

if __name__ == "__main__":
    main()
