from __future__ import annotations

import logging
import sys
from typing import Any, MutableMapping
import structlog

# Redact common PHI / sensitive fields from structured logs
SENSITIVE_KEYS = {
    "password",
    "token",
    "authorization",
    "device_token",
    "child_name",
    "patient_name",
    "dob",
    "mrn",
}


def phi_redaction_filter(
    _: Any, __: str, event_dict: MutableMapping[str, Any]
) -> MutableMapping[str, Any]:
    for key, val in list(event_dict.items()):
        if any(s in key.lower() for s in SENSITIVE_KEYS):
            event_dict[key] = "[REDACTED]"
    return event_dict


def setup_logging(debug: bool = False) -> None:
    log_level = logging.DEBUG if debug else logging.INFO

    shared_processors: list[Any] = [
        structlog.contextvars.merge_contextvars,
        structlog.processors.add_log_level,
        structlog.processors.TimeStamper(fmt="iso"),
        phi_redaction_filter,
    ]

    structlog.configure(
        processors=shared_processors + [
            structlog.processors.StackInfoRenderer(),
            structlog.processors.format_exc_info,
            structlog.processors.JSONRenderer() if not debug else structlog.dev.ConsoleRenderer(),
        ],
        wrapper_class=structlog.make_filtering_bound_logger(log_level),
        context_class=dict,
        logger_factory=structlog.PrintLoggerFactory(file=sys.stdout),
        cache_logger_on_first_use=True,
    )


logger = structlog.get_logger()
