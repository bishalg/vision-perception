from __future__ import annotations

import uuid
from datetime import datetime
from typing import Any, Optional
from sqlalchemy import DateTime, ForeignKey, JSON, String
from sqlalchemy.orm import Mapped, mapped_column

from app.db.base import Base


class CareEvent(Base):
    __tablename__ = "care_events"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=lambda: str(uuid.uuid4()))
    org_id: Mapped[str] = mapped_column(String(64), ForeignKey("organizations.id"), nullable=False, index=True)
    participant_id: Mapped[str] = mapped_column(String(64), ForeignKey("participants.id"), nullable=False, index=True)
    kind: Mapped[str] = mapped_column(String(32), nullable=False)  # task, medication, diary, milestone
    occurred_at: Mapped[datetime] = mapped_column(DateTime, default=datetime.utcnow, index=True)
    payload: Mapped[dict[str, Any]] = mapped_column(JSON, default=dict)
    idempotency_key: Mapped[Optional[str]] = mapped_column(String(128), unique=True, nullable=True)
