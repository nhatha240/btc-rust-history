"""
app.inference.postprocess — Map inference output to AiPredictionMsg.

Takes the raw runner output (direction, confidence, raw_score) plus the
originating FeatureVector and builds a wire-ready AiPredictionMsg.
"""
from __future__ import annotations

import time

from hft_proto.ai.types import AiPredictionMsg, Direction
from hft_features.types import FeatureVector


def build_prediction(
    fv: FeatureVector,
    direction: Direction,
    confidence: float,
    raw_score: float,
    model_version: str,
) -> AiPredictionMsg:
    """Build an ``AiPredictionMsg`` ready for Kafka serialisation.

    The ``ts`` field is copied from the source FeatureVector so downstream
    consumers can trace a prediction back to its originating bar.

    Args:
        fv:            originating feature vector
        direction:     predicted direction from runner
        confidence:    probability of predicted class
        raw_score:     raw model score (logged, not acted on)
        model_version: version string from config

    Returns:
        AiPredictionMsg ready to call ``.to_bytes()`` on.
    """
    return AiPredictionMsg(
        symbol=fv.symbol,
        ts=fv.ts,
        direction=direction,
        confidence=confidence,
        model_version=model_version,
        raw_score=raw_score,
    )
