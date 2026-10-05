#!/usr/bin/env python3
"""Train the coarse facial age-band classifier for the ASD subject-age check.

Produces a 4-band classifier (toddler / preschool / child / teenAdult) from a
small MobileNetV3-Small backbone. The band boundaries are deliberately coarse:
public age models overestimate young children by years, so the shipped feature
only consumes a coarse band, never an exact age.

Dataset expectations (UTKFace aligned layout):
    data/utkface/<age>_<gender>_<race>_<timestamp>.jpg   # age in years, 0-116
    data/adience/...                                     # optional, age-range dirs

Usage:
    python tools/asd_age_model/train.py --data-dir data/utkface --epochs 12
"""

from __future__ import annotations

import argparse
import random
import re
from collections import Counter
from pathlib import Path

import torch
import torch.nn as nn
from torch.utils.data import DataLoader, Dataset, WeightedRandomSampler
from torchvision import transforms
from torchvision.models import mobilenet_v3_small, MobileNet_V3_Small_Weights
from PIL import Image

# Band order MUST match the CoreML export and the Swift FacialAgeBand enum.
BANDS = ["toddler", "preschool", "child", "teenAdult"]

_UTK_RE = re.compile(r"^(\d+)_(\d)_(\d)_")

# ImageNet stats used by the torchvision backbone (baked into the exported graph).
IMAGENET_MEAN = [0.485, 0.456, 0.406]
IMAGENET_STD = [0.229, 0.224, 0.225]


def band_from_years(years: int) -> str | None:
    """Maps UTKFace age (years) to a band; returns None for out-of-scope faces."""
    if years < 1 or years > 90:  # infants & noisy extreme labels are out of scope
        return None
    if years <= 3:
        return "toddler"
    if years <= 6:
        return "preschool"
    if years <= 12:
        return "child"
    return "teenAdult"


class UTKFaceBands(Dataset):
    def __init__(self, files: list[tuple[Path, int]], train: bool):
        self.files = files
        self.transform = self._transforms(train)

    @staticmethod
    def _transforms(train: bool):
        base = [
            transforms.Resize((224, 224)),
            transforms.ToTensor(),
            transforms.Normalize(IMAGENET_MEAN, IMAGENET_STD),
        ]
        if train:
            return transforms.Compose(
                [
                    transforms.RandomResizedCrop(224, scale=(0.75, 1.0)),
                    transforms.RandomHorizontalFlip(),
                    transforms.ColorJitter(0.1, 0.1, 0.1),
                ]
                + base[1:]
            )
        return transforms.Compose(base)

    def __len__(self) -> int:
        return len(self.files)

    def __getitem__(self, index: int):
        path, label = self.files[index]
        image = Image.open(path).convert("RGB")
        return self.transform(image), label


def load_utkface(data_dir: Path) -> list[tuple[Path, int]]:
    files: list[tuple[Path, int]] = []
    for path in sorted(data_dir.rglob("*.jpg")):
        match = _UTK_RE.match(path.name)
        if not match:
            continue
        band = band_from_years(int(match.group(1)))
        if band is None:
            continue
        files.append((path, BANDS.index(band)))
    return files


def balanced_sampler(labels: list[int]) -> WeightedRandomSampler:
    counts = Counter(labels)
    weights = [1.0 / counts[label] for label in labels]
    return WeightedRandomSampler(weights, num_samples=len(labels), replacement=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-dir", type=Path, required=True, help="UTKFace jpg root")
    parser.add_argument("--output", type=Path, default=Path("models/asd_age/age_band_mobilenetv3.pt"))
    parser.add_argument("--epochs", type=int, default=12)
    parser.add_argument("--batch-size", type=int, default=128)
    parser.add_argument("--val-fraction", type=float, default=0.05)
    parser.add_argument("--seed", type=int, default=2026)
    args = parser.parse_args()

    random.seed(args.seed)
    torch.manual_seed(args.seed)

    samples = load_utkface(args.data_dir)
    if len(samples) < 1000:
        raise SystemExit(f"Too few usable samples ({len(samples)}) — check --data-dir")
    random.shuffle(samples)

    val_count = max(200, int(len(samples) * args.val_fraction))
    val_samples = samples[:val_count]
    train_samples = samples[val_count:]

    print(f"samples: {len(train_samples)} train / {len(val_samples)} val")
    print("band distribution:", dict(sorted(Counter(label for _, label in samples).items())))

    train_set = UTKFaceBands(train_samples, train=True)
    val_set = UTKFaceBands(val_samples, train=False)
    train_loader = DataLoader(
        train_set, batch_size=args.batch_size, sampler=balanced_sampler([l for _, l in train_samples]),
        num_workers=4, pin_memory=True,
    )
    val_loader = DataLoader(val_set, batch_size=args.batch_size, num_workers=2)

    device = torch.device("mps" if torch.backends.mps.is_available() else "cpu")
    model = mobilenet_v3_small(weights=MobileNet_V3_Small_Weights.IMAGENET1K_V1)
    model.classifier[3] = nn.Linear(model.classifier[3].in_features, len(BANDS))
    model.to(device)

    criterion = nn.CrossEntropyLoss(label_smoothing=0.05)
    optimizer = torch.optim.AdamW(model.parameters(), lr=3e-4, weight_decay=1e-4)
    scheduler = torch.optim.lr_scheduler.CosineAnnealingLR(optimizer, T_max=args.epochs)

    best_val_acc = 0.0
    for epoch in range(args.epochs):
        model.train()
        running_loss = 0.0
        for images, labels in train_loader:
            images, labels = images.to(device), labels.to(device)
            optimizer.zero_grad()
            loss = criterion(model(images), labels)
            loss.backward()
            optimizer.step()
            running_loss += loss.item() * images.size(0)

        model.eval()
        correct = total = 0
        confusion = [[0] * len(BANDS) for _ in BANDS]
        with torch.no_grad():
            for images, labels in val_loader:
                predictions = model(images.to(device)).argmax(dim=1).cpu()
                for pred, truth in zip(predictions.tolist(), labels.tolist()):
                    confusion[truth][pred] += 1
                    correct += int(pred == truth)
                    total += 1
        val_acc = correct / max(total, 1)
        print(
            f"epoch {epoch + 1}/{args.epochs} "
            f"loss={running_loss / max(len(train_samples), 1):.4f} val_acc={val_acc:.4f}"
        )

        # Toddler-vs-teenAdult separation is the deployment-critical metric.
        toddler_recall = confusion[0][0] / max(sum(confusion[0]), 1)
        adult_recall = confusion[3][3] / max(sum(confusion[3]), 1)
        print(f"  toddler recall={toddler_recall:.3f} teenAdult recall={adult_recall:.3f}")
        if val_acc > best_val_acc:
            best_val_acc = val_acc
            args.output.parent.mkdir(parents=True, exist_ok=True)
            torch.save(model.state_dict(), args.output)

    print(f"best val_acc={best_val_acc:.4f} → {args.output}")


if __name__ == "__main__":
    main()
