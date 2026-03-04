"""Tests for hft_proto message types and serialization."""
import pytest

from hft_proto.common.types import EnvelopeMsg, SCHEMA_VERSION_MD_FEATURE_STATE
from hft_proto.md.types import FeatureStateMsg
from hft_proto.ai.types import AiPredictionMsg, Direction


class TestEnvelopeMsg:
    def test_bytes_roundtrip(self):
        env = EnvelopeMsg(
            schema_id=SCHEMA_VERSION_MD_FEATURE_STATE,
            payload=b"\x01\x02\x03\x04",
            produced_at_ms=1_700_000_000_000,
            trace_id="test-trace-123",
        )
        raw = env.to_bytes()
        env2 = EnvelopeMsg.from_bytes(raw)
        assert env2.schema_id == env.schema_id
        assert env2.payload == env.payload
        assert env2.produced_at_ms == env.produced_at_ms
        assert env2.trace_id == env.trace_id

    def test_default_fields_auto_populated(self):
        env = EnvelopeMsg(schema_id="test.v1", payload=b"hello")
        assert env.produced_at_ms > 0
        assert len(env.trace_id) == 36  # UUID format


class TestAiPredictionMsg:
    def make(self, direction=Direction.LONG) -> AiPredictionMsg:
        return AiPredictionMsg(
            symbol="BTCUSDT",
            ts=1_700_000_000_000,
            direction=direction,
            confidence=0.87,
            model_version="lgbm-v1.0",
            raw_score=0.87,
        )

    def test_bytes_roundtrip(self):
        msg = self.make()
        msg2 = AiPredictionMsg.from_bytes(msg.to_bytes())
        assert msg2.symbol == msg.symbol
        assert msg2.ts == msg.ts
        assert msg2.direction == Direction.LONG
        assert abs(msg2.confidence - 0.87) < 1e-10

    def test_json_roundtrip(self):
        msg = self.make(Direction.SHORT)
        msg2 = AiPredictionMsg.from_json(msg.to_json())
        assert msg2.direction == Direction.SHORT
        assert msg2.model_version == "lgbm-v1.0"

    def test_direction_from_str(self):
        assert Direction.from_str("long") == Direction.LONG
        assert Direction.from_str("SHORT") == Direction.SHORT
        assert Direction.from_str("hold") == Direction.HOLD

    def test_direction_values(self):
        assert int(Direction.HOLD) == 0
        assert int(Direction.LONG) == 1
        assert int(Direction.SHORT) == 2
