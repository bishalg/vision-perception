"""Map a flood alert recommendation onto the same green / yellow / red language as the gaze app."""

ALERT_TO_INDICATOR = {
    "NORMAL": "green",
    "WARNING": "yellow",
    "CRITICAL": "red",
}


def status_indicator(alert_recommendation: str) -> str:
    """Return green, yellow, or red for a known alert level."""
    try:
        return ALERT_TO_INDICATOR[alert_recommendation]
    except KeyError as exc:
        known = ", ".join(sorted(ALERT_TO_INDICATOR))
        raise ValueError(f"alert_recommendation must be one of {known}") from exc
