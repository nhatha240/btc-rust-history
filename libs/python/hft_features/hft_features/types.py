"""
hft_features.types — FeatureVector dataclass.

This is the canonical Python representation of the feature state published
on TOPIC_FEATURE_STATE and stored in the ClickHouse `feature_state` table.

Field order matches the feature vector consumed by ML models:
  [ema_fast, ema_slow, rsi, macd, macd_signal, macd_hist, vwap]
"""
from __future__ import annotations

import time
from dataclasses import dataclass, field, fields

import numpy as np

from hft_proto.md.types import FeatureStateMsg

# Ordered list of numeric feature field names (used for to_array / from_array)
FEATURE_FIELDS: tuple[str, ...] = (
    "ema_fast",
    "ema_slow",
    "rsi",
    "macd",
    "macd_signal",
    "macd_hist",
    "vwap",
)
N_FEATURES = len(FEATURE_FIELDS)


@dataclass
class FeatureVector:
    """Canonical Python feature vector for ML inference.

    Contains the same fields as ``FeatureStateMsg`` but is the interface
    used exclusively within Python code (inference, validation, normalizer).
    """

    symbol: str
    ts: int           # bar close time — unix milliseconds

    # Numeric features — order must match FEATURE_FIELDS
    ema_fast: float
    ema_slow: float
    rsi: float        # [0, 100]
    macd: float
    macd_signal: float
    macd_hist: float
    vwap: float

    received_at: float = field(default_factory=time.time)  # processing timestamp

    # ------------------------------------------------------------------
    # Conversions
    # ------------------------------------------------------------------

    def to_array(self) -> np.ndarray:
        """Return feature values as a 1D float64 numpy array (ordered by FEATURE_FIELDS)."""
        return np.array(
            [getattr(self, f) for f in FEATURE_FIELDS],
            dtype=np.float64,
        )

    @classmethod
    def from_array(cls, symbol: str, ts: int, arr: np.ndarray) -> "FeatureVector":
        """Construct from a numpy array (must match FEATURE_FIELDS order)."""
        if arr.shape != (N_FEATURES,):
            raise ValueError(
                f"Expected array shape ({N_FEATURES},), got {arr.shape}"
            )
        kwargs = {f: float(arr[i]) for i, f in enumerate(FEATURE_FIELDS)}
        return cls(symbol=symbol, ts=ts, **kwargs)

    @classmethod
    def from_proto(cls, msg: FeatureStateMsg) -> "FeatureVector":
        """Convert a FeatureStateMsg (from Kafka) to a FeatureVector."""
        return cls(
            symbol=msg.symbol,
            ts=msg.ts,
            ema_fast=msg.ema_fast,
            ema_slow=msg.ema_slow,
            rsi=msg.rsi,
            macd=msg.macd,
            macd_signal=msg.macd_signal,
            macd_hist=msg.macd_hist,
            vwap=msg.vwap,
        )

    def to_proto(self) -> FeatureStateMsg:
        """Convert back to FeatureStateMsg for re-publishing."""
        return FeatureStateMsg(
            symbol=self.symbol,
            ts=self.ts,
            ema_fast=self.ema_fast,
            ema_slow=self.ema_slow,
            rsi=self.rsi,
            macd=self.macd,
            macd_signal=self.macd_signal,
            macd_hist=self.macd_hist,
            vwap=self.vwap,
        )

    def latency_ms(self) -> float:
        """Return processing lag in milliseconds (now - ts)."""
        return (self.received_at * 1000) - self.ts
