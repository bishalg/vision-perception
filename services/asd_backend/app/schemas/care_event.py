from __future__ import annotations

from typing import Any, Optional
from pydantic import BaseModel


class CareSyncItem(BaseModel):
    id: Optional[str] = None
    child_id: str
    kind: str  # task, medication, diary, milestone
    occurred_at: Optional[str] = None
    payload: dict[str, Any] = {}


class CareSyncBatchRequest(BaseModel):
    child_id: Optional[str] = None
    events: list[CareSyncItem] = []


class CareSyncResponse(BaseModel):
    status: str = "ok"
    synced_count: int = 0
