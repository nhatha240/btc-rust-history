"""
hft_ai_runtime — Shared async runtime for hft Python services.

Modules:
    config      — Load runtime config from env vars
    consumer    — AsyncKafkaConsumer wrapping confluent-kafka
    producer    — AsyncKafkaProducer wrapping confluent-kafka
    logging     — Structured JSON logging via structlog
    metrics     — Prometheus metrics (counters, histograms)
    health      — Lightweight aiohttp health-check server
"""

from .config import RuntimeConfig
from .consumer import AsyncKafkaConsumer
from .health import HealthServer
from .logging import setup_logging
from .metrics import RuntimeMetrics
from .producer import AsyncKafkaProducer

__all__ = [
    "RuntimeConfig",
    "AsyncKafkaConsumer",
    "AsyncKafkaProducer",
    "setup_logging",
    "RuntimeMetrics",
    "HealthServer",
]
