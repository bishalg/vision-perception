from __future__ import annotations

import uuid
from datetime import datetime
from typing import Any, Optional
from sqlalchemy import DateTime, Integer, JSON, String
from sqlalchemy.orm import Mapped, mapped_column

from app.db.base import Base


class Organization(Base):
    __tablename__ = "organizations"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=lambda: str(uuid.uuid4()))
    name: Mapped[str] = mapped_column(String(255), nullable=False)
    kind: Mapped[str] = mapped_column(String(32), default="clinic")  # home, clinic, research
    retention_months: Mapped[int] = mapped_column(Integer, default=24)
    settings: Mapped[Optional[dict[str, Any]]] = mapped_column(JSON, default=dict)
    created_at: Mapped[datetime] = mapped_column(DateTime, default=datetime.utcnow)
