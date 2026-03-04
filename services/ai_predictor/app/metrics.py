"""
app.metrics — Prometheus metrics for ai_predictor.

Extends RuntimeMetrics with service-specific gauges.
"""
from __future__ import annotations

from prometheus_client import Gauge

from hft_ai_runtime.metrics import RuntimeMetrics, get_metrics

# Gauge: model info (version label)
_model_info_gauge: Gauge | None = None


def get_ai_metrics(service_name: str = "ai_predictor") -> RuntimeMetrics:
    """Return the shared RuntimeMetrics singleton for this service."""
    return get_metrics(service_name)


def register_model_info(model_version: str) -> None:
    """Register a gauge with model_version label (visible in Grafana)."""
    global _model_info_gauge
    if _model_info_gauge is None:
        _model_info_gauge = Gauge(
            "ai_predictor_model_info",
            "Model version info",
            ["model_version"],
        )
    _model_info_gauge.labels(model_version=model_version).set(1)
