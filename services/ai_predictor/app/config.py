"""
app.config — ai_predictor service configuration.

Extends RuntimeConfig with service-specific settings.
"""
from __future__ import annotations

import os
from dataclasses import dataclass, field

from hft_ai_runtime.config import RuntimeConfig


def _env(key: str, default: str = "") -> str:
    return os.environ.get(key, default)


def _env_float(key: str, default: float) -> float:
    return float(os.environ.get(key, default))


@dataclass(frozen=True)
class AiPredictorConfig(RuntimeConfig):
    """Configuration for the ai_predictor service.

    Inherits all Kafka / logging / health settings from RuntimeConfig
    and adds model-specific settings.

    Environment variables (beyond RuntimeConfig):
        MODEL_PATH           Path to the pickled sklearn model artifact
        NORMALIZER_PATH      Path to the normalizer JSON artifact
        MODEL_VERSION        String version tag embedded in predictions
        CONFIDENCE_THRESHOLD Minimum confidence to emit a non-HOLD signal
        SERVICE_NAME         Override service name in logs (default: ai_predictor)
    """

    model_path: str = field(
        default_factory=lambda: _env("MODEL_PATH", "artifacts/model.joblib")
    )
    normalizer_path: str = field(
        default_factory=lambda: _env("NORMALIZER_PATH", "artifacts/normalizer.json")
    )
    model_version: str = field(
        default_factory=lambda: _env("MODEL_VERSION", "sklearn-v0.1.0")
    )
    confidence_threshold: float = field(
        default_factory=lambda: _env_float("CONFIDENCE_THRESHOLD", 0.6)
    )

    @classmethod
    def from_env(cls) -> "AiPredictorConfig":  # type: ignore[override]
        return cls(service_name=_env("SERVICE_NAME", "ai_predictor"))
