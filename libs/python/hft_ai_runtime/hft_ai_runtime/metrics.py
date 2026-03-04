"""
hft_ai_runtime.metrics — Prometheus metrics for hft Python services.

Exposes a ``RuntimeMetrics`` singleton.
Metrics are served by a separate HTTP server on METRICS_PORT (default 8091).

Counters / Histograms exposed:
  hft_messages_consumed_total{service, topic}
  hft_messages_produced_total{service, topic}
  hft_inference_duration_seconds{service}
  hft_validation_errors_total{service, reason}
  hft_consumer_errors_total{service, topic}
  hft_producer_errors_total{service, topic}
"""
from __future__ import annotations

import threading
from dataclasses import dataclass, field
from typing import Optional

from prometheus_client import (
    REGISTRY,
    CollectorRegistry,
    Counter,
    Histogram,
    start_http_server,
)

_INFERENCE_BUCKETS = (0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0)


@dataclass
class RuntimeMetrics:
    """Prometheus metrics bundle for an hft Python service.

    Instantiate once at startup with ``RuntimeMetrics(service_name)``.
    """

    service_name: str
    _registry: CollectorRegistry = field(default_factory=lambda: REGISTRY, init=False, repr=False)

    # ── Counters ─────────────────────────────────────────────────────
    messages_consumed: Counter = field(init=False)
    messages_produced: Counter = field(init=False)
    validation_errors: Counter = field(init=False)
    consumer_errors: Counter = field(init=False)
    producer_errors: Counter = field(init=False)

    # ── Histograms ───────────────────────────────────────────────────
    inference_duration: Histogram = field(init=False)

    def __post_init__(self) -> None:
        svc = self.service_name
        self.messages_consumed = Counter(
            "hft_messages_consumed_total",
            "Total Kafka messages consumed",
            ["service", "topic"],
        )
        self.messages_produced = Counter(
            "hft_messages_produced_total",
            "Total Kafka messages produced",
            ["service", "topic"],
        )
        self.validation_errors = Counter(
            "hft_validation_errors_total",
            "Feature vector validation failures",
            ["service", "reason"],
        )
        self.consumer_errors = Counter(
            "hft_consumer_errors_total",
            "Kafka consumer errors",
            ["service", "topic"],
        )
        self.producer_errors = Counter(
            "hft_producer_errors_total",
            "Kafka producer delivery failures",
            ["service", "topic"],
        )
        self.inference_duration = Histogram(
            "hft_inference_duration_seconds",
            "ML model inference latency",
            ["service"],
            buckets=_INFERENCE_BUCKETS,
        )

    # ------------------------------------------------------------------
    # Convenience label-bound helpers
    # ------------------------------------------------------------------

    def inc_consumed(self, topic: str) -> None:
        self.messages_consumed.labels(service=self.service_name, topic=topic).inc()

    def inc_produced(self, topic: str) -> None:
        self.messages_produced.labels(service=self.service_name, topic=topic).inc()

    def inc_validation_error(self, reason: str) -> None:
        self.validation_errors.labels(service=self.service_name, reason=reason).inc()

    def inc_consumer_error(self, topic: str) -> None:
        self.consumer_errors.labels(service=self.service_name, topic=topic).inc()

    def inc_producer_error(self, topic: str) -> None:
        self.producer_errors.labels(service=self.service_name, topic=topic).inc()

    def observe_inference(self, duration_s: float) -> None:
        self.inference_duration.labels(service=self.service_name).observe(duration_s)

    # ------------------------------------------------------------------
    # HTTP server
    # ------------------------------------------------------------------

    def start_http_server(self, port: int, addr: str = "0.0.0.0") -> None:
        """Start Prometheus scrape endpoint in a background daemon thread."""
        start_http_server(port, addr=addr)


def get_metrics(service_name: str) -> RuntimeMetrics:
    """Module-level singleton factory — safe to call multiple times."""
    global _INSTANCE
    if _INSTANCE is None:
        _INSTANCE = RuntimeMetrics(service_name=service_name)
    return _INSTANCE


_INSTANCE: Optional[RuntimeMetrics] = None
