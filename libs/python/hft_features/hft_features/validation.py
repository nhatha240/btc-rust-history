"""
hft_features.validation — Validate a FeatureVector before inference.

Checks:
  - No NaN or Inf values across all numeric fields
  - RSI in [0, 100]
  - VWAP > 0
  - EMA values > 0
"""
from __future__ import annotations

import math
from typing import Optional

from .types import FEATURE_FIELDS, FeatureVector

_RSI_MIN = 0.0
_RSI_MAX = 100.0


class FeatureValidationError(ValueError):
    """Raised when a FeatureVector fails validation."""

    def __init__(self, symbol: str, ts: int, reason: str) -> None:
        self.symbol = symbol
        self.ts = ts
        self.reason = reason
        super().__init__(f"[{symbol}@{ts}] {reason}")


def validate(fv: FeatureVector) -> None:
    """Validate a FeatureVector in-place.

    Raises:
        FeatureValidationError: if any check fails.
    """
    for fname in FEATURE_FIELDS:
        val = getattr(fv, fname)
        if math.isnan(val):
            raise FeatureValidationError(fv.symbol, fv.ts, f"field '{fname}' is NaN")
        if math.isinf(val):
            raise FeatureValidationError(fv.symbol, fv.ts, f"field '{fname}' is Inf")

    if not (_RSI_MIN <= fv.rsi <= _RSI_MAX):
        raise FeatureValidationError(
            fv.symbol, fv.ts, f"RSI={fv.rsi:.4f} out of range [{_RSI_MIN}, {_RSI_MAX}]"
        )

    if fv.vwap <= 0:
        raise FeatureValidationError(
            fv.symbol, fv.ts, f"VWAP={fv.vwap:.6f} must be > 0"
        )

    if fv.ema_fast <= 0:
        raise FeatureValidationError(
            fv.symbol, fv.ts, f"ema_fast={fv.ema_fast:.6f} must be > 0"
        )

    if fv.ema_slow <= 0:
        raise FeatureValidationError(
            fv.symbol, fv.ts, f"ema_slow={fv.ema_slow:.6f} must be > 0"
        )

    if fv.ts <= 0:
        raise FeatureValidationError(
            fv.symbol, fv.ts, "ts must be a positive unix millisecond timestamp"
        )


def is_valid(fv: FeatureVector) -> tuple[bool, Optional[str]]:
    """Non-raising version of ``validate``.

    Returns:
        (True, None) if valid, (False, reason_str) if invalid.
    """
    try:
        validate(fv)
        return True, None
    except FeatureValidationError as exc:
        return False, exc.reason
