#!/usr/bin/env python3
"""
Generate publication-grade App Store screenshots for:
1. PalmReader (iPhone 16 Pro Max 1290x2796)
2. EarlyGaze (iPhone 16 Pro Max 1290x2796 & iPad Pro 12.9" 2048x2732)
Locales: en-US, hi, ne-NP
"""

import os
from PIL import Image, ImageDraw, ImageFont

FONT_TITLE_PATH = "/System/Library/Fonts/Supplemental/Arial.ttf"
FONT_HEADING_PATH = "/System/Library/Fonts/HelveticaNeue.ttc"

def get_font(size, bold=False):
    try:
        if os.path.exists(FONT_TITLE_PATH):
            return ImageFont.truetype(FONT_TITLE_PATH, size)
        return ImageFont.load_default()
    except Exception:
        return ImageFont.load_default()

def draw_header(draw, width, badge_text, title_text, subtitle_text, accent_color=(56, 189, 248)):
    # Category badge
    f_badge = get_font(int(width * 0.024))
    f_title = get_font(int(width * 0.046))
    f_sub = get_font(int(width * 0.026))

    top_y = int(width * 0.10)

    # Badge Pill
    badge_w = len(badge_text) * int(width * 0.016) + 40
    badge_h = int(width * 0.045)
    badge_x = (width - badge_w) // 2
    draw.rounded_rectangle([badge_x, top_y, badge_x + badge_w, top_y + badge_h], radius=badge_h // 2,
                           fill=(15, 23, 42), outline=accent_color, width=2)
    draw.text((badge_x + 20, top_y + int(badge_h * 0.15)), badge_text, font=f_badge, fill=accent_color)

    # Title
    t_bbox = draw.textbbox((0, 0), title_text, font=f_title)
    tw = t_bbox[2] - t_bbox[0]
    draw.text(((width - tw) // 2, top_y + badge_h + 30), title_text, font=f_title, fill=(255, 255, 255))

    # Subtitle
    s_bbox = draw.textbbox((0, 0), subtitle_text, font=f_sub)
    sw = s_bbox[2] - s_bbox[0]
    draw.text(((width - sw) // 2, top_y + badge_h + 100), subtitle_text, font=f_sub, fill=(148, 163, 184))

def draw_footer(draw, width, height, footer_text="100% On-Device Processing • Zero Video Stored • Medical Privacy"):
    f_foot = get_font(int(width * 0.022))
    bbox = draw.textbbox((0, 0), footer_text, font=f_foot)
    fw = bbox[2] - bbox[0]
    y = height - int(height * 0.05)
    draw.text(((width - fw) // 2, y), footer_text, font=f_foot, fill=(100, 116, 139))

def create_card(width, height, bg_gradient_start, bg_gradient_end):
    img = Image.new("RGB", (width, height), bg_gradient_start)
    draw = ImageDraw.Draw(img)
    # subtle gradient
    for y in range(height):
        r = int(bg_gradient_start[0] + (bg_gradient_end[0] - bg_gradient_start[0]) * (y / height))
        g = int(bg_gradient_start[1] + (bg_gradient_end[1] - bg_gradient_start[1]) * (y / height))
        b = int(bg_gradient_start[2] + (bg_gradient_end[2] - bg_gradient_start[2]) * (y / height))
        draw.line([(0, y), (width, y)], fill=(r, g, b))
    return img, draw

def generate_palm_screenshots(out_dir):
    locales = {
        "en-US": [
            ("AI COMPUTER VISION", "Real-Time Optical Palm Scanner", "Intelligent hand detection & live guidance overlay", "Scanner"),
            ("CREASE EXTRACTION", "Major Hand Line Detection", "Traces Heart, Head, Life and Fate lines accurately", "Creases"),
            ("PRECISION TUNING", "Interactive Spline Loupe Editor", "Inspect fine branches, forks, and crease islands", "Editor"),
            ("ANCIENT WISDOM", "Vedic Samudrika Shastra", "Planetary mount analysis: Jupiter, Saturn, Sun & Moon", "Samudrika"),
            ("BIOMETRIC ANALYSIS", "Hand Anthropometry & Typology", "Finger length ratios, 2D:4D index & mount density", "Biometrics"),
        ],
        "hi": [
            ("एआई कंप्यूटर विज़न", "रीयल-टाइम पाम स्कैनर", "सटीक हाथ की पहचान और लाइव गाइडेंस", "Scanner"),
            ("हस्तरेखा विश्लेषण", "प्रमुख रेखाओं का स्वचालित मापन", "हृदय, मस्तिष्क, जीवन और भाग्य रेखा की पहचान", "Creases"),
            ("सटीक संपादन", "इंटरेक्टिव रेखा संपादक लेंस", "सूक्ष्म शाखाओं और पर्वतों की गहन जांच", "Editor"),
            ("वैदिक सामुद्रिक शास्त्र", "ग्रह पर्वतों का शास्त्रीय फल", "गुरु, शनि, सूर्य, बुध, शुक्र व चंद्र पर्वत का अध्ययन", "Samudrika"),
            ("बायोमेट्रिक माप", "हाथ की संरचना और उंगलियों का अनुपात", "2D:4D अनुपात और अग्नि/वायु/जल प्रकृति विश्लेषण", "Biometrics"),
        ],
        "ne-NP": [
            ("एआई प्रविधि", "प्रत्यक्ष क्यामेरा पाम स्क्यानर", "हातको रेखा र बनावटको तत्काल सटीक पहिचान", "Scanner"),
            ("हस्तरेखा पहिचान", "प्रमुख रेखाहरूको विश्लेषण", "हृदय, मस्तिष्क, जीवन र भाग्य रेखाको रेखांकन", "Creases"),
            ("सूक्ष्म अवलोकन", "इन्टरेक्टिभ म्याग्निफाइङ लेन्स", "रेखाहरूका शाखा र द्वीपहरूको स्पष्ट अवलोकन", "Editor"),
            ("वैदिक फलकथन", "सामुद्रिक शास्त्र र ग्रह पर्वत", "बृहस्पति, शनि, सूर्य र चन्द्र पर्वतको फल", "Samudrika"),
            ("बायोमेट्रिक बनावट", "हत्केला र औंलाको अनुपात", "औंलाहरूको अनुपात र हातको प्रकृति विश्लेषण", "Biometrics"),
        ]
    }

    width, height = 1290, 2796
    device_prefix = "iPhone 16 Pro Max"

    for loc, screens in locales.items():
        loc_dir = os.path.join(out_dir, loc)
        os.makedirs(loc_dir, exist_ok=True)

        for idx, (badge, title, sub, label) in enumerate(screens, 1):
            img, draw = create_card(width, height, (10, 15, 30), (5, 8, 18))

            draw_header(draw, width, badge, title, sub, accent_color=(245, 158, 11))
            draw_footer(draw, width, height, "100% On-Device Processing • Zero Video Stored • Offline Capable")

            # Mockup Phone Container
            frame_x1 = int(width * 0.10)
            frame_x2 = int(width * 0.90)
            frame_y1 = int(height * 0.22)
            frame_y2 = int(height * 0.91)
            frame_w = frame_x2 - frame_x1
            frame_h = frame_y2 - frame_y1

            # Outer phone border
            draw.rounded_rectangle([frame_x1, frame_y1, frame_x2, frame_y2], radius=60, fill=(15, 23, 42), outline=(51, 65, 85), width=6)
            # Screen inset
            pad = 20
            draw.rounded_rectangle([frame_x1 + pad, frame_y1 + pad, frame_x2 - pad, frame_y2 - pad], radius=44, fill=(2, 6, 23))

            # Inner mock visuals based on screen index
            inner_cx = (frame_x1 + frame_x2) // 2
            inner_cy = (frame_y1 + frame_y2) // 2

            if idx == 1: # Scanner
                # Hand guide box
                guide_w, guide_h = int(frame_w * 0.7), int(frame_h * 0.55)
                gx1 = inner_cx - guide_w // 2
                gy1 = inner_cy - guide_h // 2 - 40
                draw.rounded_rectangle([gx1, gy1, gx1 + guide_w, gy1 + guide_h], radius=30, outline=(34, 197, 94), width=5)
                # Corner reticles
                for cx, cy in [(gx1, gy1), (gx1 + guide_w, gy1), (gx1, gy1 + guide_h), (gx1 + guide_w, gy1 + guide_h)]:
                    draw.arc([cx - 20, cy - 20, cx + 20, cy + 20], 0, 360, fill=(34, 197, 94), width=4)
                # Status pill
                pill_y = gy1 + guide_h + 60
                draw.rounded_rectangle([inner_cx - 180, pill_y, inner_cx + 180, pill_y + 60], radius=30, fill=(34, 197, 94, 50), outline=(34, 197, 94), width=2)
                f_s = get_font(26)
                draw.text((inner_cx - 120, pill_y + 15), "PALM LOCKED (99.2%)", font=f_s, fill=(255, 255, 255))

            elif idx == 2: # Creases
                # Render palm lines
                cy = inner_cy - 100
                draw.line([(inner_cx - 200, cy - 80), (inner_cx - 50, cy - 120), (inner_cx + 150, cy - 140)], fill=(239, 68, 68), width=8) # Heart
                draw.line([(inner_cx - 200, cy), (inner_cx - 20, cy + 30), (inner_cx + 180, cy + 80)], fill=(59, 130, 246), width=8) # Head
                draw.arc([inner_cx - 250, cy - 20, inner_cx + 100, cy + 320], 30, 160, fill=(245, 158, 11), width=8) # Life
                draw.line([(inner_cx + 10, cy + 260), (inner_cx + 20, cy - 60)], fill=(168, 85, 247), width=6) # Fate

                # Legend Box
                leg_y = inner_cy + 240
                draw.rounded_rectangle([frame_x1 + 60, leg_y, frame_x2 - 60, leg_y + 220], radius=20, fill=(30, 41, 59), outline=(71, 85, 105), width=2)
                f_l = get_font(24)
                draw.text((frame_x1 + 90, leg_y + 25), "• Heart Line: Deep & Empathetic", font=f_l, fill=(239, 68, 68))
                draw.text((frame_x1 + 90, leg_y + 75), "• Head Line: Analytical & Focused", font=f_l, fill=(59, 130, 246))
                draw.text((frame_x1 + 90, leg_y + 125), "• Life Line: High Vitality & Balance", font=f_l, fill=(245, 158, 11))
                draw.text((frame_x1 + 90, leg_y + 175), "• Fate Line: Strong Career Trajectory", font=f_l, fill=(168, 85, 247))

            elif idx == 3: # Editor & Loupe
                # Loupe Circle
                lx, ly, lr = inner_cx, inner_cy - 80, 180
                draw.ellipse([lx - lr, ly - lr, lx + lr, ly + lr], outline=(245, 158, 11), fill=(15, 23, 42), width=6)
                # magnified spline
                draw.line([(lx - 140, ly + 60), (lx - 40, ly - 20), (lx + 80, ly - 60), (lx + 140, ly - 90)], fill=(59, 130, 246), width=10)
                # Control vertices
                for vx, vy in [(lx - 140, ly + 60), (lx - 40, ly - 20), (lx + 80, ly - 60), (lx + 140, ly - 90)]:
                    draw.ellipse([vx - 14, vy - 14, vx + 14, vy + 14], fill=(245, 158, 11), outline=(255, 255, 255), width=3)
                # Loupe badge
                f_loupe = get_font(22)
                draw.text((lx - 100, ly + lr + 40), "MAGNIFYING LOUPE (3.0X)", font=f_loupe, fill=(245, 158, 11))

            elif idx == 4: # Samudrika Mounts
                card_y = inner_cy - 260
                draw.rounded_rectangle([frame_x1 + 50, card_y, frame_x2 - 50, card_y + 540], radius=24, fill=(30, 41, 59), outline=(245, 158, 11), width=3)
                f_h = get_font(28)
                f_b = get_font(22)
                draw.text((frame_x1 + 80, card_y + 30), "Vedic Planetary Mount Profile", font=f_h, fill=(245, 158, 11))
                mounts = [
                    ("Jupiter Mount (Guru):", "Elevated - Strong leadership & ambition"),
                    ("Sun Mount (Surya):", "Well-defined - High creative expression"),
                    ("Venus Mount (Shukra):", "Prominent - Refined artistic appreciation"),
                    ("Mercury Mount (Budha):", "Clear - Agile analytical communication"),
                    ("Saturn Mount (Shani):", "Balanced - Methodical perseverance")
                ]
                for m_i, (m_name, m_val) in enumerate(mounts):
                    my = card_y + 90 + (m_i * 85)
                    draw.text((frame_x1 + 80, my), m_name, font=f_b, fill=(255, 255, 255))
                    draw.text((frame_x1 + 80, my + 30), m_val, font=f_b, fill=(148, 163, 184))

            elif idx == 5: # Biometrics
                card_y = inner_cy - 240
                draw.rounded_rectangle([frame_x1 + 50, card_y, frame_x2 - 50, card_y + 500], radius=24, fill=(30, 41, 59), outline=(56, 189, 248), width=3)
                f_h = get_font(28)
                f_b = get_font(22)
                draw.text((frame_x1 + 80, card_y + 30), "Hand Biometric Anthropometry", font=f_h, fill=(56, 189, 248))
                metrics = [
                    ("2D:4D Digit Ratio:", "0.962 (Normalized balanced marker)"),
                    ("Palm Aspect Ratio:", "1.08 (Square / Earth element hand)"),
                    ("Mount Elevation Index:", "+2.4mm above basilar plane"),
                    ("Skin Crease Density:", "42.8 crossings per cm²"),
                    ("Hand Typology:", "Prithvi (Earth) - Pragmatic & Grounded")
                ]
                for m_i, (m_name, m_val) in enumerate(metrics):
                    my = card_y + 90 + (m_i * 75)
                    draw.text((frame_x1 + 80, my), m_name, font=f_b, fill=(255, 255, 255))
                    draw.text((frame_x1 + 80, my + 28), m_val, font=f_b, fill=(148, 163, 184))

            out_path = os.path.join(loc_dir, f"{device_prefix}-{idx:02d}_{label}.png")
            img.save(out_path, "PNG")
            print(f"Saved: {out_path}")

def generate_asd_gaze_screenshots(out_dir):
    locales = {
        "en-US": [
            ("CLINICAL DECISION SUPPORT", "Passive Toddler Gaze Screener", "Non-invasive optical attention tracking for 12-48 months", "Screening"),
            ("TODDLER CALIBRATION", "5-Point Bubble Popping Game", "Gentle, child-friendly spatial eye calibration in seconds", "Calibration"),
            ("OBJECTIVE BIOMARKERS", "16 Clinical Digital Biomarkers", "Validated social engagement, latency & fixation ratios", "Biomarkers"),
            ("VISUAL ATTENTION HEATMAP", "Real-Time Gaze Replay & Heatmap", "Precise coordinate tracking over pediatric video stimuli", "Heatmap"),
            ("PEDIATRICIAN REPORT", "Clinical Doctor PDF Summaries", "Normative age comparisons & actionable diagnostic support", "DoctorReport"),
        ],
        "hi": [
            ("क्लिनिकल स्क्रीनिंग", "शिशु दृष्टि एवं ध्यान ट्रैकर", "12-48 महीने के बच्चों के लिए अहिंसक आई-ट्रैकिंग", "Screening"),
            ("सहज कैलिब्रेशन", "5-पॉइंट बबल फोड़ने वाला खेल", "बच्चों के अनुकूल कुछ ही सेकंड में दृष्टि कैलिब्रेशन", "Calibration"),
            ("वस्तुपरक बायोमार्कर्स", "16 नैदानिक डिजिटल बायोमार्कर्स", "सामाजिक प्राथमिकता, ध्यान गति व स्थायित्व का सटीक मापन", "Biomarkers"),
            ("दृष्टि हीटमैप", "रीयल-टाइम विजुअल हीटमैप रिप्ले", "बाल कार्टून वीडियो पर दृष्टि ठहराव का स्पष्ट दृश्य", "Heatmap"),
            ("डॉक्टर रिपोर्ट", "बाल रोग विशेषज्ञ पीडीएफ रिपोर्ट", "आयु-आधारित तुलना और संपूर्ण नैदानिक विश्लेषण", "DoctorReport"),
        ],
        "ne-NP": [
            ("क्लिनिकल स्क्रिनिङ", "बालबालिका दृष्टि ट्र्याकिङ", "१२-४८ महिनाका बालबालिकाका लागि अहिंसक स्क्रिनिङ", "Screening"),
            ("सरल क्यालिब्रेसन", "५-बिन्दु बबल फुटाउने खेल", "बालबालिकाले खेल्दै केही सेकेन्डमै सहज क्यालिब्रेसन", "Calibration"),
            ("क्लिनिकल बायोमार्कर्स", "१६ वस्तुपरक डिजिटल बायोमार्कर्स", "सामाजिक ध्यान, प्रतिक्रिया गति र स्थायित्वको मापन", "Biomarkers"),
            ("दृष्टि विश्लेषण", "रीयल-टाइम दृष्टि हीटम्याप रिप्ले", "भिडियो हेर्दा आँखा कहाँ अडियो भन्ने स्पष्ट दृश्य", "Heatmap"),
            ("चिकित्सक प्रतिवेदन", "बालरोग विशेषज्ञ पीडीएफ प्रतिवेदन", "उमेर अनुसार तुलना र डाक्टरसँग छलफल गर्न उपयोगी रिपोर्ट", "DoctorReport"),
        ]
    }

    # Generate both iPhone and iPad sets
    targets = [
        ("iPhone 16 Pro Max", 1290, 2796),
        ("iPad Pro (12.9-inch)", 2048, 2732)
    ]

    for dev_name, width, height in targets:
        for loc, screens in locales.items():
            loc_dir = os.path.join(out_dir, loc)
            os.makedirs(loc_dir, exist_ok=True)

            for idx, (badge, title, sub, label) in enumerate(screens, 1):
                img, draw = create_card(width, height, (8, 28, 36), (4, 15, 20))

                draw_header(draw, width, badge, title, sub, accent_color=(20, 184, 166))
                draw_footer(draw, width, height, "100% Video-Free Medical Privacy • TrueDepth On-Device Processing • Zero Video Stored")

                # Mockup Frame
                fx1 = int(width * 0.10)
                fx2 = int(width * 0.90)
                fy1 = int(height * 0.22)
                fy2 = int(height * 0.91)
                fw = fx2 - fx1
                fh = fy2 - fy1

                draw.rounded_rectangle([fx1, fy1, fx2, fy2], radius=50, fill=(15, 23, 42), outline=(19, 78, 74), width=6)
                pad = 18
                draw.rounded_rectangle([fx1 + pad, fy1 + pad, fx2 - pad, fy2 - pad], radius=36, fill=(2, 6, 23))

                icx = (fx1 + fx2) // 2
                icy = (fy1 + fy2) // 2

                if idx == 1: # Screening (GeoPref split)
                    # Split screen stimulus simulation
                    split_w, split_h = int(fw * 0.85), int(fh * 0.45)
                    sx1 = icx - split_w // 2
                    sy1 = icy - split_h // 2 - 40
                    mid_x = sx1 + split_w // 2
                    # Left: Social video
                    draw.rectangle([sx1, sy1, mid_x, sy1 + split_h], fill=(30, 58, 138))
                    # Right: Geometric video
                    draw.rectangle([mid_x, sy1, sx1 + split_w, sy1 + split_h], fill=(88, 28, 135))
                    # Labels
                    f_stim = get_font(int(width * 0.022))
                    draw.text((sx1 + 40, sy1 + 30), "Social Interaction Video", font=f_stim, fill=(255, 255, 255))
                    draw.text((mid_x + 40, sy1 + 30), "Dynamic Geometric Shapes", font=f_stim, fill=(255, 255, 255))
                    # Gaze points (cyan)
                    draw.ellipse([sx1 + 160, sy1 + 140, sx1 + 220, sy1 + 200], fill=(20, 184, 166, 120), outline=(45, 212, 191), width=4)
                    draw.ellipse([sx1 + 180, sy1 + 160, sx1 + 200, sy1 + 180], fill=(255, 255, 255))
                    # Ratio pill
                    pill_y = sy1 + split_h + 50
                    draw.rounded_rectangle([icx - 220, pill_y, icx + 220, pill_y + 64], radius=32, fill=(19, 78, 74), outline=(45, 212, 191), width=2)
                    draw.text((icx - 180, pill_y + 18), "SOCIAL PREFERENCE: 72.4%", font=f_stim, fill=(255, 255, 255))

                elif idx == 2: # Bubble Calibration
                    cy = icy - 40
                    f_cal = get_font(int(width * 0.024))
                    draw.text((icx - 170, cy - 240), "Tap each bubble to calibrate", font=f_cal, fill=(204, 251, 241))
                    bubbles = [
                        (icx - 240, cy - 120, "1"),
                        (icx + 240, cy - 120, "2"),
                        (icx, cy, "3"),
                        (icx - 240, cy + 120, "4"),
                        (icx + 240, cy + 120, "5")
                    ]
                    for bx, by, num in bubbles:
                        draw.ellipse([bx - 60, by - 60, bx + 60, by + 60], fill=(13, 148, 136), outline=(45, 212, 191), width=4)
                        draw.text((bx - 10, by - 16), num, font=f_cal, fill=(255, 255, 255))

                elif idx == 3: # 16 Biomarkers Dashboard
                    cy = icy - 260
                    card_w = fw - 80
                    draw.rounded_rectangle([icx - card_w//2, cy, icx + card_w//2, cy + 540], radius=24, fill=(15, 23, 42), outline=(20, 184, 166), width=3)
                    f_h = get_font(int(width * 0.026))
                    f_b = get_font(int(width * 0.020))
                    draw.text((icx - card_w//2 + 40, cy + 25), "Validated Clinical Digital Biomarkers", font=f_h, fill=(45, 212, 191))
                    bio = [
                        ("Social Visual Preference:", "72.4% (Typical Peer Mean: 68-76%)"),
                        ("First Eye Fixation Latency:", "290 ms (Speed to engage eyes)"),
                        ("Eye / Mouth Dwell Ratio:", "3.25 (Normative balance index)"),
                        ("Geometric Dwell Ratio:", "27.6% (Below 69% risk threshold)"),
                        ("Synchronized Smile Latency:", "480 ms (Affective contagion)")
                    ]
                    for b_i, (b_name, b_val) in enumerate(bio):
                        by = cy + 85 + (b_i * 85)
                        draw.text((icx - card_w//2 + 40, by), b_name, font=f_b, fill=(255, 255, 255))
                        draw.text((icx - card_w//2 + 40, by + 30), b_val, font=f_b, fill=(148, 163, 184))

                elif idx == 4: # Heatmap
                    hm_w, hm_h = int(fw * 0.85), int(fh * 0.45)
                    hx1 = icx - hm_w // 2
                    hy1 = icy - hm_h // 2 - 20
                    draw.rectangle([hx1, hy1, hx1 + hm_w, hy1 + hm_h], fill=(30, 41, 59))
                    # Gaze Heatmap clusters
                    draw.ellipse([hx1 + 140, hy1 + 100, hx1 + 340, hy1 + 300], fill=(239, 68, 68, 140)) # Hotspot
                    draw.ellipse([hx1 + 180, hy1 + 140, hx1 + 300, hy1 + 260], fill=(245, 158, 11, 180)) # Intense
                    draw.ellipse([hx1 + 220, hy1 + 180, hx1 + 260, hy1 + 220], fill=(255, 255, 255, 240)) # Core
                    f_hm = get_font(int(width * 0.022))
                    draw.text((hx1 + 30, hy1 + 30), "Gaze Fixation Density Overlay", font=f_hm, fill=(255, 255, 255))

                elif idx == 5: # Doctor Report PDF Preview
                    cy = icy - 260
                    card_w = fw - 100
                    draw.rounded_rectangle([icx - card_w//2, cy, icx + card_w//2, cy + 540], radius=16, fill=(255, 255, 255))
                    f_rp = get_font(int(width * 0.024))
                    f_rb = get_font(int(width * 0.018))
                    draw.text((icx - card_w//2 + 40, cy + 30), "CLINICAL ASD SCREENING SUMMARY", font=f_rp, fill=(15, 23, 42))
                    draw.text((icx - card_w//2 + 40, cy + 70), "Pediatric Development & Gaze Assessment", font=f_rb, fill=(71, 85, 105))
                    draw.line([(icx - card_w//2 + 40, cy + 105), (icx + card_w//2 - 40, cy + 105)], fill=(203, 213, 225), width=2)
                    rows = [
                        "Participant ID: STUDY-ASD-1002 | Age: 24 Months",
                        "Screening Paradigm: GeoPref Dual Video (30s)",
                        "Social Visual Preference: 72% [Typical Range]",
                        "M-CHAT-R Total Failed: 1 (Low Risk Profile)",
                        "Standard Coding: LOINC 80628-1, SNOMED 713444003",
                        "Recommendation: Routine 24-month developmental surveillance"
                    ]
                    for r_i, row in enumerate(rows):
                        draw.text((icx - card_w//2 + 40, cy + 130 + (r_i * 55)), row, font=f_rb, fill=(30, 41, 59))

                out_path = os.path.join(loc_dir, f"{dev_name}-{idx:02d}_{label}.png")
                img.save(out_path, "PNG")
                print(f"Saved: {out_path}")

def main():
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    palm_screenshots = os.path.join(base_dir, "apps", "ios_palm_app", "fastlane", "screenshots")
    asd_screenshots = os.path.join(base_dir, "apps", "ios_asd_gaze_app", "fastlane", "screenshots")

    print("Generating PalmReader screenshots...")
    generate_palm_screenshots(palm_screenshots)

    print("\nGenerating EarlyGaze screenshots...")
    generate_asd_gaze_screenshots(asd_screenshots)

    print("\nAll publication-grade screenshots generated successfully!")

if __name__ == "__main__":
    main()
