from __future__ import annotations

import uuid
from datetime import datetime
from typing import Any, Optional
from sqlalchemy import DateTime, JSON, String, Text
from sqlalchemy.orm import Mapped, mapped_column

from app.db.base import Base


class ConfigDocument(Base):
    __tablename__ = "config_documents"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=lambda: str(uuid.uuid4()))
    kind: Mapped[str] = mapped_column(String(64), nullable=False)  # stimulus_catalog, normative_baselines, scoring_profile
    version: Mapped[str] = mapped_column(String(32), nullable=False)
    status: Mapped[str] = mapped_column(String(32), default="draft")  # draft, candidate, published, retired
    body: Mapped[dict[str, Any]] = mapped_column(JSON, default=dict)
    signature: Mapped[Optional[str]] = mapped_column(String(255), nullable=True)
    created_by: Mapped[Optional[str]] = mapped_column(String(64), nullable=True)
    created_at: Mapped[datetime] = mapped_column(DateTime, default=datetime.utcnow)
    published_at: Mapped[Optional[datetime]] = mapped_column(DateTime, nullable=True)
    notes: Mapped[Optional[str]] = mapped_column(Text, nullable=True)


class ConfigBundle(Base):
    __tablename__ = "config_bundles"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=lambda: str(uuid.uuid4()))
    catalog_id: Mapped[str] = mapped_column(String(64), nullable=False)
    baselines_id: Mapped[str] = mapped_column(String(64), nullable=False)
    scoring_id: Mapped[str] = mapped_column(String(64), nullable=False)
    bundle_version: Mapped[str] = mapped_column(String(32), nullable=False, unique=True)
    signature: Mapped[str] = mapped_column(String(255), nullable=False)
    published_at: Mapped[datetime] = mapped_column(DateTime, default=datetime.utcnow)
