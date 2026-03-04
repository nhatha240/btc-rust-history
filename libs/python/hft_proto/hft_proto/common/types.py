"""
hft_proto.common.types — Envelope dataclass (protobuf-compatible).

This module provides a pure-Python dataclass mirror of the Envelope proto.
Use this when protoc-generated stubs are not available. When protoc is set up,
prefer importing from `hft_proto.common.envelope_pb2` instead.
"""
from __future__ import annotations

import time
import uuid
from dataclasses import dataclass, field
from typing import Type, TypeVar

T = TypeVar("T")

SCHEMA_VERSION_MD_FEATURE_STATE = "md.FeatureState.v1"
SCHEMA_VERSION_AI_PREDICTION = "ai.AiPrediction.v1"


@dataclass
class EnvelopeMsg:
    """Wire envelope for all Kafka messages.

    Wraps a serialized protobuf payload with schema metadata and trace info.
    Mirrors ``common.Envelope`` from proto/common/envelope.proto.
    """

    schema_id: str
    payload: bytes
    produced_at_ms: int = field(default_factory=lambda: int(time.time() * 1000))
    trace_id: str = field(default_factory=lambda: str(uuid.uuid4()))

    # ------------------------------------------------------------------
    # Serialisation helpers (minimal — not full protobuf encoding)
    # ------------------------------------------------------------------

    def to_bytes(self) -> bytes:
        """Encode as length-prefixed binary: [schema_id len][schema_id][trace_id len][trace_id][produced_at_ms 8B][payload]."""
        import struct

        schema_b = self.schema_id.encode()
        trace_b = self.trace_id.encode()
        return (
            struct.pack(">H", len(schema_b))
            + schema_b
            + struct.pack(">H", len(trace_b))
            + trace_b
            + struct.pack(">Q", self.produced_at_ms)
            + self.payload
        )

    @classmethod
    def from_bytes(cls, data: bytes) -> "EnvelopeMsg":
        """Decode envelope produced by ``to_bytes``."""
        import struct

        offset = 0
        (schema_len,) = struct.unpack_from(">H", data, offset)
        offset += 2
        schema_id = data[offset : offset + schema_len].decode()
        offset += schema_len

        (trace_len,) = struct.unpack_from(">H", data, offset)
        offset += 2
        trace_id = data[offset : offset + trace_len].decode()
        offset += trace_len

        (produced_at_ms,) = struct.unpack_from(">Q", data, offset)
        offset += 8
        payload = data[offset:]
        return cls(
            schema_id=schema_id,
            payload=payload,
            produced_at_ms=produced_at_ms,
            trace_id=trace_id,
        )
