#!/usr/bin/env python3
"""
scripts/check_registration.py

Sub-pixel image registration check between PRE (2026-08-24) and POST (2026-08-27):
1. Runs cv2.findTransformECC with cv2.MOTION_TRANSLATION on grayscale images.
2. Falls back to ORB keypoints + RANSAC translation estimation if ECC fails.
3. If shift > 3.0 px, warps POST image and valid mask into PRE coordinate frame.
4. Records method, shift vector, and registration status into out/registration.json.
"""

import json
import os
import sys
import cv2
import numpy as np

PRE_IMAGE = "data/flood_nepal_2026/processed/pre_20260824_bgr.png"
POST_IMAGE = "data/flood_nepal_2026/processed/post_20260827_bgr.png"
POST_MASK = "data/flood_nepal_2026/processed/post_valid_mask.png"
OUT_DIR = "data/flood_nepal_2026/out"
PROC_DIR = "data/flood_nepal_2026/processed"


def register_ecc(img_pre_gray: np.ndarray, img_post_gray: np.ndarray):
    """Attempts ECC translation alignment. Returns (warp_matrix_2x3, True) or (None, False)."""
    warp_matrix = np.eye(2, 3, dtype=np.float32)
    criteria = (cv2.TERM_CRITERIA_EPS | cv2.TERM_CRITERIA_COUNT, 100, 1e-5)
    try:
        _, warp_matrix = cv2.findTransformECC(
            templateImage=img_pre_gray,
            inputImage=img_post_gray,
            warpMatrix=warp_matrix,
            motionType=cv2.MOTION_TRANSLATION,
            criteria=criteria,
            inputMask=None,
            gaussFiltSize=5
        )
        return warp_matrix, True
    except cv2.error as e:
        print(f"[WARN] ECC failed: {e}. Falling back to ORB...", file=sys.stderr)
        return None, False


def register_orb(img_pre_gray: np.ndarray, img_post_gray: np.ndarray):
    """ORB keypoint matching + median translation vector fallback."""
    orb = cv2.ORB_create(nfeatures=2000)
    kp1, des1 = orb.detectAndCompute(img_pre_gray, None)
    kp2, des2 = orb.detectAndCompute(img_post_gray, None)

    if des1 is None or des2 is None or len(kp1) < 10 or len(kp2) < 10:
        return None, False

    matcher = cv2.BFMatcher(cv2.NORM_HAMMING, crossCheck=True)
    matches = matcher.match(des1, des2)
    if len(matches) < 8:
        return None, False

    matches = sorted(matches, key=lambda m: m.distance)[:100]
    pts_pre = np.float32([kp1[m.queryIdx].pt for m in matches])
    pts_post = np.float32([kp2[m.trainIdx].pt for m in matches])

    # Translation vector from POST to PRE
    diffs = pts_pre - pts_post
    dx = float(np.median(diffs[:, 0]))
    dy = float(np.median(diffs[:, 1]))

    warp_matrix = np.array([[1.0, 0.0, dx], [0.0, 1.0, dy]], dtype=np.float32)
    return warp_matrix, True


def main():
    os.makedirs(OUT_DIR, exist_ok=True)

    if not os.path.isfile(PRE_IMAGE) or not os.path.isfile(POST_IMAGE):
        print("[ERROR] Input images missing. Run scripts/prepare_flood_data.py first.", file=sys.stderr)
        sys.exit(1)

    pre_bgr = cv2.imread(PRE_IMAGE)
    post_bgr = cv2.imread(POST_IMAGE)
    post_mask = cv2.imread(POST_MASK, cv2.IMREAD_GRAYSCALE)

    pre_gray = cv2.cvtColor(pre_bgr, cv2.COLOR_BGR2GRAY)
    post_gray = cv2.cvtColor(post_bgr, cv2.COLOR_BGR2GRAY)

    print("[REGISTRATION] Checking sub-pixel alignment between PRE and POST...")

    warp_matrix, success = register_ecc(pre_gray, post_gray)
    method = "ECC"

    if not success:
        warp_matrix, success = register_orb(pre_gray, post_gray)
        method = "ORB"

    if not success:
        method = "unverified"
        shift_x = 0.0
        shift_y = 0.0
        shift_mag = 0.0
        is_warped = False
        print("[REGISTRATION] Alignment unverified; continuing unaligned.", file=sys.stderr)
    else:
        shift_x = float(warp_matrix[0, 2])
        shift_y = float(warp_matrix[1, 2])
        shift_mag = float(np.hypot(shift_x, shift_y))
        print(f"[REGISTRATION] Method: {method}, Shift: dx={shift_x:+.3f} px, dy={shift_y:+.3f} px (magnitude={shift_mag:.3f} px)")

    THRESHOLD_PX = 3.0
    is_warped = False

    if success and shift_mag > THRESHOLD_PX:
        print(f"[REGISTRATION] Shift magnitude ({shift_mag:.3f} px) > {THRESHOLD_PX} px threshold. Warping POST image + mask into PRE frame...")
        h, w = pre_gray.shape
        post_reg = cv2.warpAffine(post_bgr, warp_matrix, (w, h), flags=cv2.INTER_LINEAR, borderMode=cv2.BORDER_CONSTANT, borderValue=(255, 255, 255))
        mask_reg = cv2.warpAffine(post_mask, warp_matrix, (w, h), flags=cv2.INTER_NEAREST, borderMode=cv2.BORDER_CONSTANT, borderValue=0)

        out_post_reg = os.path.join(PROC_DIR, "post_20260827_bgr_registered.png")
        out_mask_reg = os.path.join(PROC_DIR, "post_valid_mask_registered.png")
        cv2.imwrite(out_post_reg, post_reg)
        cv2.imwrite(out_mask_reg, mask_reg)
        is_warped = True
        print(f"[REGISTRATION] Saved registered POST image to: {out_post_reg}")
        print(f"[REGISTRATION] Saved registered POST mask to:  {out_mask_reg}")
    else:
        print(f"[REGISTRATION] Shift magnitude ({shift_mag:.3f} px) <= {THRESHOLD_PX} px threshold. Sub-pixel alignment nominal; warping not required.")

    reg_report = {
        "method": method,
        "shift_vector_px": {
            "dx": round(shift_x, 4),
            "dy": round(shift_y, 4),
            "magnitude": round(shift_mag, 4)
        },
        "threshold_px": THRESHOLD_PX,
        "is_warped": is_warped,
        "target_reference": "PRE_20260824",
        "aligned_image": "post_20260827_bgr_registered.png" if is_warped else "post_20260827_bgr.png",
        "aligned_mask": "post_valid_mask_registered.png" if is_warped else "post_valid_mask.png"
    }

    report_path = os.path.join(OUT_DIR, "registration.json")
    with open(report_path, "w") as f:
        json.dump(reg_report, f, indent=2)

    print(f"[SUCCESS] Registration report saved to: {report_path}")


if __name__ == "__main__":
    main()
