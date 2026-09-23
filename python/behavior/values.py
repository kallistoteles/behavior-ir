"""Encoding of Python input values for engine requests (data-model.md → Evaluation request)."""

from __future__ import annotations

from decimal import Decimal
from enum import Enum
from typing import Any

from .expr import normalize_decimal


def encode_value(v: Any) -> Any:
    """bool/int/str pass through; Decimal → normalized string; Enum → its value; None → null.

    Floats are rejected here, before anything reaches the engine.
    """
    if isinstance(v, float):
        raise TypeError("float values are not allowed: use decimal.Decimal for exact numbers")
    if v is None or isinstance(v, (bool, int, str)):
        return v
    if isinstance(v, Decimal):
        return normalize_decimal(v)
    if isinstance(v, Enum):
        return encode_value(v.value)
    if isinstance(v, dict):
        return {str(k): encode_value(x) for k, x in v.items()}
    raise TypeError(f"unsupported value {v!r}")


def encode_request_values(section: dict[str, Any]) -> dict[str, Any]:
    return {name: encode_value(v) for name, v in section.items()}
