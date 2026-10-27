from __future__ import annotations

import uuid
from datetime import datetime
from typing import Optional
from sqlalchemy import DateTime, ForeignKey, String
from sqlalchemy.orm import Mapped, mapped_column

from app.db.base import Base


class User(Base):
    __tablename__ = "users"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=lambda: str(uuid.uuid4()))
    org_id: Mapped[str] = mapped_column(String(64), ForeignKey("organizations.id"), nullable=False, index=True)
    email: Mapped[str] = mapped_column(String(255), unique=True, index=True, nullable=False)
    pw_hash: Mapped[str] = mapped_column(String(255), nullable=False)
    totp_secret: Mapped[Optional[str]] = mapped_column(String(128), nullable=True)
    role: Mapped[str] = mapped_column(String(32), default="clinician")  # owner, admin, clinician, researcher, reviewer, viewer, superadmin
    status: Mapped[str] = mapped_column(String(32), default="active")
    last_login_at: Mapped[Optional[datetime]] = mapped_column(DateTime, nullable=True)
    created_at: Mapped[datetime] = mapped_column(DateTime, default=datetime.utcnow)
