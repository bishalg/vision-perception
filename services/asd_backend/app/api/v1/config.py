from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Optional
from fastapi import APIRouter, Depends, Header, HTTPException, Response
from pydantic import BaseModel, Field
from sqlalchemy import select
from sqlalchemy.orm import Session

from app.db.session import get_db
from app.models import ConfigBundle

router = APIRouter(prefix="/v1/config", tags=["Configuration"])


class ConfigBundleResponse(BaseModel):
    bundle_version: str
    active_catalog_version: str = "2026.10.1"
    catalog: dict[str, Any]
    baselines: dict[str, Any]
    scoring_profile: dict[str, Any]
    feature_flags: dict[str, Any] = Field(default_factory=dict)
    signature: str


@router.get("/bundle", response_model=ConfigBundleResponse)
def get_config_bundle(
    response: Response,
    if_none_match: Optional[str] = Header(None, alias="If-None-Match"),
    db: Session = Depends(get_db),
) -> ConfigBundleResponse:
    bundle_version = "2026.10.1"
    etag = f'"{bundle_version}"'
    if if_none_match == etag:
        response.status_code = 304
        return Response(status_code=304)

    response.headers["ETag"] = etag

    # Fallback to local catalog and baseline fixtures if DB hasn't populated published documents yet
    catalog_path = Path("domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources/stimulus_catalog.json")
    baselines_path = Path("domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources/normative_baselines.json")

    catalog = {}
    if catalog_path.exists():
        with open(catalog_path, "r", encoding="utf-8") as f:
            catalog = json.load(f)

    baselines = {}
    if baselines_path.exists():
        with open(baselines_path, "r", encoding="utf-8") as f:
            baselines = json.load(f)

    scoring_profile = {
        "version": "1.0",
        "weights": {
            "socialPreference": 0.40,
            "eyeDwell": 0.30,
            "orientingLatency": 0.20,
            "affectReactivity": 0.10
        },
        "thresholds": {
            "socialLow": 0.35,
            "socialHigh": 0.65
        }
    }

    return ConfigBundleResponse(
        bundle_version=bundle_version,
        catalog=catalog,
        baselines=baselines,
        scoring_profile=scoring_profile,
        signature="ed25519_sig_provisional_bundle_v1",
    )
