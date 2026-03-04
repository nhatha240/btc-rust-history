"""
hft_proto.ai.types — AiPrediction dataclass (protobuf-compatible).

Mirrors ``ai.AiPrediction`` from proto/ai/predictions.proto.
Published on TOPIC_SIGNALS by ai_predictor.
"""
from __future__ import annotations

import json
import struct
from dataclasses import asdict, dataclass
from enum import IntEnum


class Direction(IntEnum):
    """Signal direction — matches signal_direction ENUM in PostgreSQL."""

    HOLD = 0
    LONG = 1
    SHORT = 2

    @classmethod
    def from_str(cls, s: str) -> "Direction":
        return cls[s.upper()]


@dataclass
class AiPredictionMsg:
    """AI signal published on TOPIC_SIGNALS by the ai_predictor service.

    Consumers: order_executor, paper_trader.
    """

    symbol: str
    ts: int           # unix milliseconds — matches bar ts from FeatureState
    direction: Direction
    confidence: float  # [0.0, 1.0]
    model_version: str
    raw_score: float = 0.0  # raw model output before thresholding

    # ------------------------------------------------------------------
    # Serialisation helpers
    # ------------------------------------------------------------------

    def to_bytes(self) -> bytes:
        sym_b = self.symbol.encode()
        ver_b = self.model_version.encode()
        return (
            struct.pack(">H", len(sym_b))
            + sym_b
            + struct.pack(">q", self.ts)
            + struct.pack(">B", int(self.direction))
            + struct.pack(">2d", self.confidence, self.raw_score)
            + struct.pack(">H", len(ver_b))
            + ver_b
        )

    @classmethod
    def from_bytes(cls, data: bytes) -> "AiPredictionMsg":
        offset = 0
        (sym_len,) = struct.unpack_from(">H", data, offset)
        offset += 2
        symbol = data[offset : offset + sym_len].decode()
        offset += sym_len
        (ts,) = struct.unpack_from(">q", data, offset)
        offset += 8
        (dir_int,) = struct.unpack_from(">B", data, offset)
        offset += 1
        confidence, raw_score = struct.unpack_from(">2d", data, offset)
        offset += 16
        (ver_len,) = struct.unpack_from(">H", data, offset)
        offset += 2
        model_version = data[offset : offset + ver_len].decode()
        return cls(
            symbol=symbol,
            ts=ts,
            direction=Direction(dir_int),
            confidence=confidence,
            model_version=model_version,
            raw_score=raw_score,
        )

    def to_dict(self) -> dict:
        d = asdict(self)
        d["direction"] = self.direction.name
        return d

    @classmethod
    def from_dict(cls, d: dict) -> "AiPredictionMsg":
        d = d.copy()
        d["direction"] = Direction.from_str(d["direction"])
        return cls(**d)

    def to_json(self) -> str:
        return json.dumps(self.to_dict())

    @classmethod
    def from_json(cls, s: str) -> "AiPredictionMsg":
        return cls.from_dict(json.loads(s))
