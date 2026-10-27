from __future__ import annotations

import os
from typing import Optional
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    PROJECT_NAME: str = "ASD Pediatric Attention Screening Backend"
    API_V1_PREFIX: str = "/api/v1"
    V1_PREFIX: str = "/v1"
    ENVIRONMENT: str = "development"
    DEBUG: bool = False

    # Database
    DATABASE_URL: str = "sqlite:///./asd_local.db"
    DB_ECHO: bool = False

    # Security & Auth
    JWT_SECRET_KEY: str = "insecure-local-dev-key-change-in-production-env"
    JWT_ALGORITHM: str = "HS256"
    ACCESS_TOKEN_EXPIRE_MINUTES: int = 60 * 24  # 24 hours
    DEVICE_TOKEN_SECRET: str = "insecure-device-token-secret-change-in-production"

    # Cloud Storage (S3 / MinIO)
    S3_ENDPOINT_URL: Optional[str] = None
    S3_BUCKET_SESSIONS: str = "curv-asd-sessions"
    S3_ACCESS_KEY: Optional[str] = None
    S3_SECRET_KEY: Optional[str] = None

    # Study Info (Empty by default per U4 / H-5 requirements)
    ASD_STUDY_STATUS: str = "active"
    ASD_STUDY_IDENTIFIER: str = ""
    ASD_IRB_PROTOCOL_ID: str = ""
    ASD_STUDY_INSTITUTION: str = ""
    ASD_STUDY_PI: str = ""
    ASD_CONSENT_VERSION: str = ""

    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore"
    )


settings = Settings()
