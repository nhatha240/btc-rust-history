"""
hft_proto.md.types — FeatureState dataclass (protobuf-compatible).

Mirrors ``md.FeatureState`` from proto/md/features.proto.
Fields match the feature_state ClickHouse table and TOPIC_FEATURE_STATE schema.
"""
from __future__ import annotations

import json
import struct
from dataclasses import asdict, dataclass


@dataclass
class FeatureStateMsg:
    """Feature vector published on TOPIC_FEATURE_STATE after each bar close.

    All float fields use double (64-bit) precision matching ClickHouse Float64.
    """

    symbol: str
    ts: int          # bar close time — unix milliseconds
    ema_fast: float  # Fast EMA (default period 12)
    ema_slow: float  # Slow EMA (default period 26)
    rsi: float       # RSI [0, 100]
    macd: float      # MACD line = ema_fast - ema_slow
    macd_signal: float  # Signal line = EMA(macd, 9)
    macd_hist: float    # Histogram = macd - macd_signal
    vwap: float      # Volume-Weighted Average Price

    _STRUCT_FMT = ">8d"  # 8 doubles (not symbol/ts)
    _FLOAT_FIELDS = ("ema_fast", "ema_slow", "rsi", "macd", "macd_signal", "macd_hist", "vwap")
    # ts packed separately as int64

    # ------------------------------------------------------------------
    # Serialisation helpers
    # ------------------------------------------------------------------

    def to_bytes(self) -> bytes:
        """Compact binary serialisation: symbol(len+bytes) + ts(int64) + 7 doubles."""
        sym_b = self.symbol.encode()
        header = struct.pack(">H", len(sym_b)) + sym_b + struct.pack(">q", self.ts)
        body = struct.pack(
            ">7d",
            self.ema_fast,
            self.ema_slow,
            self.rsi,
            self.macd,
            self.macd_signal,
            self.macd_hist,
            self.vwap,
        )
        return header + body

    @classmethod
    def from_bytes(cls, data: bytes) -> "FeatureStateMsg":
        offset = 0
        (sym_len,) = struct.unpack_from(">H", data, offset)
        offset += 2
        symbol = data[offset : offset + sym_len].decode()
        offset += sym_len
        (ts,) = struct.unpack_from(">q", data, offset)
        offset += 8
        ema_fast, ema_slow, rsi, macd, macd_signal, macd_hist, vwap = struct.unpack_from(
            ">7d", data, offset
        )
        return cls(
            symbol=symbol,
            ts=ts,
            ema_fast=ema_fast,
            ema_slow=ema_slow,
            rsi=rsi,
            macd=macd,
            macd_signal=macd_signal,
            macd_hist=macd_hist,
            vwap=vwap,
        )

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def from_dict(cls, d: dict) -> "FeatureStateMsg":
        return cls(**d)

    def to_json(self) -> str:
        return json.dumps(self.to_dict())

    @classmethod
    def from_json(cls, s: str) -> "FeatureStateMsg":
        return cls.from_dict(json.loads(s))
