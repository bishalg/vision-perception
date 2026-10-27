from __future__ import annotations

import hashlib
import secrets
from typing import Any, Optional
from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.orm import Session

from app.db.session import get_db
from app.models import Device, Organization

router = APIRouter(prefix="/v1/devices", tags=["Devices"])


class DeviceRegisterRequest(BaseModel):
    enrol_code: Optional[str] = None
    platform: str = "iOS"
    app_version: str = "1.0.0"
    label: Optional[str] = "Clinic iPad"


class DeviceRegisterResponse(BaseModel):
    status: str = "registered"
    device_id: str
    device_token: str
    org_id: str
    config_bundle_version: str = "2026.10.1"


@router.post("/register", response_model=DeviceRegisterResponse)
def register_device(
    payload: DeviceRegisterRequest,
    db: Session = Depends(get_db),
) -> DeviceRegisterResponse:
    org = db.scalar(select(Organization).where(Organization.kind == "home"))
    if not org:
        org = Organization(id="default-home-org", name="Default Public Org", kind="home")
        db.add(org)
        db.commit()
        db.refresh(org)

    raw_token = "curv_dev_" + secrets.token_urlsafe(32)
    token_hash = hashlib.sha256(raw_token.encode("utf-8")).hexdigest()

    device = Device(
        org_id=org.id,
        platform=payload.platform,
        app_version=payload.app_version,
        token_hash=token_hash,
        label=payload.label or "Clinic iPad",
    )
    db.add(device)
    db.commit()
    db.refresh(device)

    return DeviceRegisterResponse(
        status="registered",
        device_id=device.id,
        device_token=raw_token,
        org_id=org.id,
        config_bundle_version="2026.10.1",
    )
