from __future__ import annotations

from fastapi import APIRouter, Depends
from sqlalchemy import text
from sqlalchemy.orm import Session

from app.core.config import settings
from app.db.session import get_db
from app.schemas.common import HealthResponse

router = APIRouter(tags=["Health"])


@router.get("/health", response_model=HealthResponse)
@router.get("/v1/health", response_model=HealthResponse)
def health_check(db: Session = Depends(get_db)) -> HealthResponse:
    db_status = "connected"
    try:
        db.execute(text("SELECT 1"))
    except Exception:
        db_status = "unhealthy"

    return HealthResponse(
        status="ok" if db_status == "connected" else "degraded",
        service="asd_backend",
        environment=settings.ENVIRONMENT,
        version="0.1.0",
        database=db_status,
    )
