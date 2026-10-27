from __future__ import annotations

import uuid
from datetime import datetime
from typing import Optional
from sqlalchemy import DateTime, ForeignKey, String
from sqlalchemy.orm import Mapped, mapped_column

from app.db.base import Base


class Device(Base):
    __tablename__ = "devices"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=lambda: str(uuid.uuid4()))
    org_id: Mapped[str] = mapped_column(String(64), ForeignKey("organizations.id"), nullable=False, index=True)
    platform: Mapped[str] = mapped_column(String(32), default="iOS")
    app_version: Mapped[str] = mapped_column(String(32), default="1.0.0")
    attest_key_id: Mapped[Optional[str]] = mapped_column(String(255), nullable=True)
    token_hash: Mapped[str] = mapped_column(String(255), unique=True, index=True, nullable=False)
    label: Mapped[str] = mapped_column(String(128), default="Clinic iPad")
    enrolled_by: Mapped[Optional[str]] = mapped_column(String(64), nullable=True)
    enrolled_at: Mapped[datetime] = mapped_column(DateTime, default=datetime.utcnow)
    revoked_at: Mapped[Optional[datetime]] = mapped_column(DateTime, nullable=True)
