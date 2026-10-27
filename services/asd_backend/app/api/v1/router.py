from __future__ import annotations

from fastapi import APIRouter

from app.api.v1.health import router as health_router
from app.api.v1.shims import router as shims_router
from app.api.v1.sessions import router as sessions_router
from app.api.v1.devices import router as devices_router
from app.api.v1.config import router as config_router

api_router = APIRouter()
api_router.include_router(health_router)
api_router.include_router(shims_router)
api_router.include_router(sessions_router)
api_router.include_router(devices_router)
api_router.include_router(config_router)
