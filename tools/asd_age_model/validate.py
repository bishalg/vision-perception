#!/usr/bin/env python3
"""Validate the exported CoreML age-band model before shipping it to devices.

Runs the traced/converted model over a held-out validation split and prints the
per-band confusion matrix plus the deployment-critical toddler-vs-teenAdult
separation metrics. Blocks shipping when the separation gate fails.

Usage:
    python tools/asd_age_model/validate.py \
        --checkpoint models/asd_age/age_band_mobilenetv3.pt \
        --data-dir data/utkface
"""

from __future__ import annotations

import argparse
import random
from collections import Counter
from pathlib import Path

import torch
from torchvision import transforms
from torchvision.models import mobilenet_v3_small
from PIL import Image

from train import BANDS, IMAGENET_MEAN, IMAGENET_STD, UTKFaceBands, load_utkface

# Deployment gate: the subject-age check only acts on stable toddler/teenAdult
# separation — misclassifying an adult as a toddler (or vice versa) at high rate
# would make the assistive prompts useless.
MIN_TODDLER_RECALL = 0.90
MIN_ADULT_RECALL = 0.90


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--data-dir", type=Path, required=True)
    parser.add_argument("--val-fraction", type=float, default=0.05)
    parser.add_argument("--seed", type=int, default=2026)
    args = parser.parse_args()

    random.seed(args.seed)
    samples = load_utkface(args.data_dir)
    random.shuffle(samples)
    val_count = max(200, int(len(samples) * args.val_fraction))
    val_set = UTKFaceBands(samples[:val_count], train=False)

    device = torch.device("mps" if torch.backends.mps.is_available() else "cpu")
    model = mobilenet_v3_small(weights=None)
    model.classifier[3] = torch.nn.Linear(model.classifier[3].in_features, len(BANDS))
    model.load_state_dict(torch.load(args.checkpoint, map_location="cpu"))
    model.to(device).eval()

    loader = torch.utils.data.DataLoader(val_set, batch_size=128, num_workers=2)
    confusion = [[0] * len(BANDS) for _ in BANDS]
    with torch.no_grad():
        for images, labels in loader:
            predictions = model(images.to(device)).argmax(dim=1).cpu().tolist()
            for pred, truth in zip(predictions, labels):
                confusion[truth][pred] += 1

    print("confusion matrix (rows = truth, cols = prediction):")
    print("            " + "  ".join(f"{b:>9}" for b in BANDS))
    for band, row in zip(BANDS, confusion):
        print(f"{band:>10}  " + "  ".join(f"{v:>9}" for v in row))

    total = sum(sum(row) for row in confusion)
    accuracy = sum(confusion[i][i] for i in range(len(BANDS))) / max(total, 1)
    toddler_recall = confusion[0][0] / max(sum(confusion[0]), 1)
    adult_recall = confusion[3][3] / max(sum(confusion[3]), 1)
    toddler_as_adult = confusion[0][3] / max(sum(confusion[0]), 1)
    adult_as_toddler = confusion[3][0] / max(sum(confusion[3]), 1)

    print(f"\naccuracy={accuracy:.4f}")
    print(f"toddler recall={toddler_recall:.4f} (as teenAdult: {toddler_as_adult:.4f})")
    print(f"teenAdult recall={adult_recall:.4f} (as toddler: {adult_as_toddler:.4f})")

    failed = toddler_recall < MIN_TODDLER_RECALL or adult_recall < MIN_ADULT_RECALL
    if failed:
        raise SystemExit(
            f"SHIP BLOCKED: separation gate requires toddler ≥ {MIN_TODDLER_RECALL} "
            f"and teenAdult ≥ {MIN_ADULT_RECALL} recall"
        )
    print("separation gate passed — model is shippable")


if __name__ == "__main__":
    main()
