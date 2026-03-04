"""
hft_proto — Protobuf schemas and Python stubs for hft trading services.

Sub-packages:
  hft_proto.md      — Market data messages (FeatureState, RawTick)
  hft_proto.ai      — AI prediction messages (AiPrediction)
  hft_proto.common  — Shared types (Envelope)

Usage (using generated protobuf classes):
    from hft_proto.md import features_pb2
    msg = features_pb2.FeatureState()
    msg.symbol = "BTCUSDT"
    msg.rsi = 55.3
    raw = msg.SerializeToString()

Usage (using dataclass helpers — no protoc required):
    from hft_proto.md.types import FeatureStateMsg
    from hft_proto.ai.types import AiPredictionMsg, Direction
"""

from importlib.metadata import version, PackageNotFoundError

try:
    __version__ = version("hft_proto")
except PackageNotFoundError:
    __version__ = "0.1.0-dev"

__all__ = ["__version__"]
