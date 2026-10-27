# ASD Cloud Backend (`services/asd_backend`)

Cloud ingestion, clinical configuration delivery, and administrative API for the ASD Pediatric Attention Screening platform.

## Architecture & Decisions
- **Stack:** Python 3.9+ / 3.12, FastAPI, SQLAlchemy 2 (async + sync compatible), Alembic migrations, Pydantic v2, PostgreSQL 16 (SQLite in unit tests).
- **Two Auth Planes:**
  - *Device Plane:* Bearer device token registered via App Attest or clinic enrolment code.
  - *User Plane:* Argon2id password hash + JWT access / refresh (Cognito migration ready for Phase 3).
- **Configuration Delivery:** Signed remote config bundles (stimulus catalog, normative baselines, scoring profile).
- **Privacy Posture:** Zero raw video recorded or ingested; pseudonymous participant tokens (`child-<session-prefix>`); PHI minimization.

## Running Locally

```bash
# Using Makefile
make install
make up          # spins up PostgreSQL and backend via docker-compose
make test        # runs pytest suite with unit tests
make run         # runs local dev server via uvicorn
```

## API Surfaces

- `GET /health` - Liveness & readiness probes
- `GET /v1/config/bundle` - Signed clinical configuration bundle
- `POST /v1/devices/register` - Device plane enrolment
- `POST /v1/sessions` - Idempotent gaze observation session ingestion
- `/api/v1/research/*` & `/api/v1/carekit/*` - Backward compatibility shims for v1 iOS client builds
