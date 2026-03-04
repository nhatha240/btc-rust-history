"""
hft_ai_runtime.config — Runtime configuration loaded from environment variables.

All variables have sensible defaults for local development.
Override in Docker via docker-compose environment section.
"""
from __future__ import annotations

import os
from dataclasses import dataclass, field


def _env(key: str, default: str = "") -> str:
    return os.environ.get(key, default)


def _env_int(key: str, default: int) -> int:
    return int(os.environ.get(key, default))


def _env_float(key: str, default: float) -> float:
    return float(os.environ.get(key, default))


def _env_list(key: str, default: str = "") -> list[str]:
    val = os.environ.get(key, default)
    return [v.strip() for v in val.split(",") if v.strip()]


@dataclass(frozen=True)
class RuntimeConfig:
    """Immutable runtime configuration for hft Python services.

    Loaded from environment variables; build once at startup.

    Example (docker-compose):
        KAFKA_BROKERS=redpanda:9092
        KAFKA_GROUP_ID=ai_predictor
        KAFKA_INPUT_TOPICS=TOPIC_FEATURE_STATE
        KAFKA_OUTPUT_TOPIC=TOPIC_SIGNALS
        HEALTH_PORT=8090
        LOG_LEVEL=INFO
    """

    # ── Kafka ───────────────────────────────────────────────────────────
    kafka_brokers: str = field(default_factory=lambda: _env("KAFKA_BROKERS", "localhost:9092"))
    kafka_group_id: str = field(default_factory=lambda: _env("KAFKA_GROUP_ID", "hft_ai_runtime"))
    kafka_input_topics: list[str] = field(
        default_factory=lambda: _env_list("KAFKA_INPUT_TOPICS", "TOPIC_FEATURE_STATE")
    )
    kafka_output_topic: str = field(
        default_factory=lambda: _env("KAFKA_OUTPUT_TOPIC", "TOPIC_SIGNALS")
    )
    kafka_auto_offset_reset: str = field(
        default_factory=lambda: _env("KAFKA_AUTO_OFFSET_RESET", "latest")
    )
    kafka_session_timeout_ms: int = field(
        default_factory=lambda: _env_int("KAFKA_SESSION_TIMEOUT_MS", 30_000)
    )
    kafka_fetch_min_bytes: int = field(
        default_factory=lambda: _env_int("KAFKA_FETCH_MIN_BYTES", 1)
    )
    kafka_producer_acks: str = field(
        default_factory=lambda: _env("KAFKA_PRODUCER_ACKS", "all")
    )
    kafka_producer_linger_ms: int = field(
        default_factory=lambda: _env_int("KAFKA_PRODUCER_LINGER_MS", 5)
    )

    # ── Service ─────────────────────────────────────────────────────────
    service_name: str = field(default_factory=lambda: _env("SERVICE_NAME", "hft_service"))
    health_port: int = field(default_factory=lambda: _env_int("HEALTH_PORT", 8090))
    health_host: str = field(default_factory=lambda: _env("HEALTH_HOST", "0.0.0.0"))

    # ── Observability ────────────────────────────────────────────────────
    log_level: str = field(default_factory=lambda: _env("LOG_LEVEL", "INFO"))
    metrics_port: int = field(default_factory=lambda: _env_int("METRICS_PORT", 8091))
    otel_endpoint: str = field(
        default_factory=lambda: _env("OTEL_EXPORTER_OTLP_ENDPOINT", "")
    )

    # ------------------------------------------------------------------
    # Helpers
    # ------------------------------------------------------------------

    def kafka_consumer_conf(self) -> dict:
        """Return a confluent-kafka consumer config dict."""
        return {
            "bootstrap.servers": self.kafka_brokers,
            "group.id": self.kafka_group_id,
            "auto.offset.reset": self.kafka_auto_offset_reset,
            "session.timeout.ms": self.kafka_session_timeout_ms,
            "fetch.min.bytes": self.kafka_fetch_min_bytes,
            "enable.auto.commit": True,
        }

    def kafka_producer_conf(self) -> dict:
        """Return a confluent-kafka producer config dict."""
        return {
            "bootstrap.servers": self.kafka_brokers,
            "acks": self.kafka_producer_acks,
            "linger.ms": self.kafka_producer_linger_ms,
            "compression.type": "snappy",
        }

    @classmethod
    def from_env(cls) -> "RuntimeConfig":
        """Construct from environment variables (default constructor)."""
        return cls()
