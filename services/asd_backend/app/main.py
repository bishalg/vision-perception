from __future__ import annotations

from contextlib import asynccontextmanager
from typing import AsyncGenerator
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from app.api.v1.router import api_router
from app.core.config import settings
from app.core.logging import logger, setup_logging
from app.db.base import Base
from app.db.session import engine


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncGenerator[None, None]:
    setup_logging(debug=settings.DEBUG)
    logger.info("Starting ASD Pediatric Screening Backend", environment=settings.ENVIRONMENT)

    # In local development or SQLite test mode, create tables if they do not exist
    if settings.DATABASE_URL.startswith("sqlite"):
        Base.metadata.create_all(bind=engine)

    yield

    logger.info("Shutting down ASD Pediatric Screening Backend")


app = FastAPI(
    title=settings.PROJECT_NAME,
    version="0.1.0",
    lifespan=lifespan,
)

# CORS configuration
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Include all API routes
app.include_router(api_router)

