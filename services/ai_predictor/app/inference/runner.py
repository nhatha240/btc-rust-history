"""
app.inference.runner — Run ML inference on a FeatureVector.

The runner:
  1. Retrieves the singleton ModelLoader
  2. Normalizes the feature array
  3. Calls model.predict_proba() (or predict() for non-probabilistic models)
  4. Returns (direction, confidence, raw_score)

Inference is synchronous (sklearn is CPU-bound) and run via run_in_executor
in the async context to avoid blocking the event loop.
"""
from __future__ import annotations

import time
from typing import TYPE_CHECKING

import numpy as np
import structlog

from hft_proto.ai.types import Direction
from hft_features.types import FeatureVector

from app.inference.model_loader import ModelLoader
from app.metrics import get_ai_metrics

if TYPE_CHECKING:
    from app.config import AiPredictorConfig

logger = structlog.get_logger(__name__)

# Class index mapping from sklearn model output → Direction
# Assumes model was trained with labels: [0=HOLD, 1=LONG, 2=SHORT]
_CLASS_TO_DIRECTION = {
    0: Direction.HOLD,
    1: Direction.LONG,
    2: Direction.SHORT,
}


def run_inference(
    fv: FeatureVector,
    config: "AiPredictorConfig",
) -> tuple[Direction, float, float]:
    """Synchronous inference — call via asyncio.run_in_executor.

    Returns:
        (direction, confidence, raw_score)
          direction:  predicted Direction enum
          confidence: probability of predicted class [0, 1]
          raw_score:  argmax raw score (for logging)
    """
    metrics = get_ai_metrics(config.service_name)
    loader = ModelLoader.instance()

    t0 = time.perf_counter()

    # Normalize
    norm_arr = loader.normalizer.transform_feature_vector(fv)
    X = norm_arr.reshape(1, -1)

    # Predict
    model = loader.model
    if hasattr(model, "predict_proba"):
        proba = model.predict_proba(X)[0]  # shape (n_classes,)
        class_idx = int(np.argmax(proba))
        confidence = float(proba[class_idx])
        raw_score = float(proba[class_idx])
    else:
        # Fallback: deterministic predict, confidence fixed at 1.0
        class_idx = int(model.predict(X)[0])
        confidence = 1.0
        raw_score = float(class_idx)

    elapsed = time.perf_counter() - t0
    metrics.observe_inference(elapsed)

    direction = _CLASS_TO_DIRECTION.get(class_idx, Direction.HOLD)

    # Apply confidence threshold — emit HOLD if below threshold
    if direction != Direction.HOLD and confidence < config.confidence_threshold:
        logger.debug(
            "confidence_below_threshold",
            symbol=fv.symbol,
            direction=direction.name,
            confidence=confidence,
            threshold=config.confidence_threshold,
        )
        direction = Direction.HOLD

    logger.info(
        "inference_complete",
        symbol=fv.symbol,
        ts=fv.ts,
        direction=direction.name,
        confidence=f"{confidence:.4f}",
        latency_ms=f"{elapsed * 1000:.2f}",
    )
    return direction, confidence, raw_score
