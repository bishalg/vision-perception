from __future__ import annotations

import uuid
from datetime import datetime
from typing import Optional
from sqlalchemy import DateTime, ForeignKey, String
from sqlalchemy.orm import Mapped, mapped_column

from app.db.base import Base


class Participant(Base):
    __tablename__ = "participants"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=lambda: str(uuid.uuid4()))
    org_id: Mapped[str] = mapped_column(String(64), ForeignKey("organizations.id"), nullable=False, index=True)
    token: Mapped[str] = mapped_column(String(64), unique=True, index=True, nullable=False)  # child-<id> token
    age_bracket_at_enrol: Mapped[str] = mapped_column(String(32), default="age_12_24")
    language: Mapped[str] = mapped_column(String(8), default="en")
    consent_version: Mapped[str] = mapped_column(String(32), default="1.0")
    consent_at: Mapped[datetime] = mapped_column(DateTime, default=datetime.utcnow)
    mode: Mapped[str] = mapped_column(String(16), default="parent")  # parent, clinic
    withdrawn_at: Mapped[Optional[datetime]] = mapped_column(DateTime, nullable=True)
