#!/usr/bin/env python3
"""Convert the trained age-band checkpoint to a CoreML package.

Wraps the backbone with a baked-in ImageNet normalization module so the exported
CoreML model takes a plain 224×224 image (scale 1/255) and emits classifier
outputs compatible with `AgeBandEstimationEngine`:

    input:  "image"                   (224×224 color)
    output: "probabilities"           dictionary keyed by FacialAgeBand.rawValue
            "classLabel"              top band string

Usage:
    python tools/asd_age_model/convert_coreml.py \
        --checkpoint models/asd_age/age_band_mobilenetv3.pt \
        --output models/asd_age/AgeBand.mlpackage

Then compile for on-device use:
    xcrun coremlcompiler compile models/asd_age/AgeBand.mlpackage <dest-dir>
"""

from __future__ import annotations

import argparse
from pathlib import Path

import coremltools as ct
import torch
import torch.nn as nn
from torchvision.models import mobilenet_v3_small, MobileNet_V3_Small_Weights

BANDS = ["toddler", "preschool", "child", "teenAdult"]
IMAGENET_MEAN = [0.485, 0.456, 0.406]
IMAGENET_STD = [0.229, 0.224, 0.225]


class Normalize(nn.Module):
    """Bakes ImageNet normalization into the graph so CoreML needs no per-channel bias."""

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        mean = torch.tensor(IMAGENET_MEAN, device=x.device).view(1, 3, 1, 1)
        std = torch.tensor(IMAGENET_STD, device=x.device).view(1, 3, 1, 1)
        return (x - mean) / std


def build_model(checkpoint: Path | None) -> nn.Module:
    model = mobilenet_v3_small(weights=None)
    model.classifier[3] = nn.Linear(model.classifier[3].in_features, len(BANDS))
    if checkpoint is not None:
        model.load_state_dict(torch.load(checkpoint, map_location="cpu"))
        print(f"loaded checkpoint {checkpoint}")
    else:
        # Untrained export (useful for wiring up the app pipeline end-to-end).
        print("WARNING: no checkpoint given — exporting randomly-initialized weights")
    model.eval()
    return nn.Sequential(Normalize(), model)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", type=Path, default=None)
    parser.add_argument("--output", type=Path, default=Path("models/asd_age/AgeBand.mlpackage"))
    args = parser.parse_args()

    model = build_model(args.checkpoint)
    traced = torch.jit.trace(model, torch.zeros(1, 3, 224, 224))

    classifier_config = ct.ClassifierConfig(class_labels=BANDS, predicted_probabilities_output="probabilities")
    mlmodel = ct.convert(
        traced,
        inputs=[ct.ImageType(name="image", shape=(1, 3, 224, 224), scale=1.0 / 255.0)],
        classifier_config=classifier_config,
        compute_precision=ct.precision.FLOAT16,
        convert_to="mlprogram",
        minimum_deployment_target=ct.target.iOS17,
    )
    mlmodel.author = "Curv ASD Screening"
    mlmodel.short_description = (
        "Assistive coarse facial age-band classifier (toddler/preschool/child/teenAdult). "
        "Subject-verification UX signal only — NOT age verification."
    )
    mlmodel.input_description["image"] = "Upright 224×224 face crop"
    mlmodel.output_description["probabilities"] = "Per-band probabilities"
    mlmodel.output_description["classLabel"] = "Modal age band"

    args.output.parent.mkdir(parents=True, exist_ok=True)
    mlmodel.save(str(args.output))
    print(f"saved {args.output}")
    print("compile with: xcrun coremlcompiler compile "
          f"{args.output} <destination-dir>  # produces AgeBand.mlmodelc")


if __name__ == "__main__":
    main()
