from __future__ import annotations

import uuid
from datetime import datetime
from typing import Any, Optional
from sqlalchemy import DateTime, Integer, JSON, String
from sqlalchemy.orm import Mapped, mapped_column

from app.db.base import Base


class AuditLog(Base):
    __tablename__ = "audit_log"

    id: Mapped[int] = mapped_column(Integer, primary_key=True, autoincrement=True)
    ts: Mapped[datetime] = mapped_column(DateTime, default=datetime.utcnow, index=True)
    actor_type: Mapped[str] = mapped_column(String(32), default="device")  # user, device, system
    actor_id: Mapped[str] = mapped_column(String(64), nullable=False)
    org_id: Mapped[Optional[str]] = mapped_column(String(64), nullable=True, index=True)
    action: Mapped[str] = mapped_column(String(64), nullable=False)
    target_type: Mapped[str] = mapped_column(String(64), nullable=False)
    target_id: Mapped[str] = mapped_column(String(64), nullable=False)
    before: Mapped[Optional[dict[str, Any]]] = mapped_column(JSON, nullable=True)
    after: Mapped[Optional[dict[str, Any]]] = mapped_column(JSON, nullable=True)
    ip: Mapped[Optional[str]] = mapped_column(String(64), nullable=True)
    user_agent: Mapped[Optional[str]] = mapped_column(String(255), nullable=True)


class ReviewItem(Base):
    __tablename__ = "review_items"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=lambda: str(uuid.uuid4()))
    org_id: Mapped[str] = mapped_column(String(64), nullable=False, index=True)
    session_id: Mapped[str] = mapped_column(String(64), nullable=False, index=True)
    reason: Mapped[str] = mapped_column(String(64), nullable=False)  # low_quality, flag_red, random_audit, norm_candidate
    status: Mapped[str] = mapped_column(String(32), default="open", index=True)  # open, in_review, done, dismissed
    assigned_to: Mapped[Optional[str]] = mapped_column(String(64), nullable=True)
    decision: Mapped[Optional[dict[str, Any]]] = mapped_column(JSON, nullable=True)
    decided_by: Mapped[Optional[str]] = mapped_column(String(64), nullable=True)
    decided_at: Mapped[Optional[datetime]] = mapped_column(DateTime, nullable=True)
