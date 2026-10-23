import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from flood_status import status_indicator


def test_maps_alert_levels_onto_traffic_lights():
    assert status_indicator("NORMAL") == "green"
    assert status_indicator("WARNING") == "yellow"
    assert status_indicator("CRITICAL") == "red"


def test_unknown_alert_is_rejected():
    with pytest.raises(ValueError):
        status_indicator("PANIC")
