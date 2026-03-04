"""
hft_ai_runtime.logging — Structured JSON logging via structlog.

Call ``setup_logging(level, service_name)`` once at service startup.
After that, get a logger with ``structlog.get_logger()``.

Output format: newline-delimited JSON on stdout, compatible with
Grafana Loki / Cloud Logging log ingestion.

Example output:
  {"event": "consumer_started", "topics": ["TOPIC_FEATURE_STATE"],
   "service": "ai_predictor", "level": "info", "timestamp": "2026-03-05T..."}
"""
from __future__ import annotations

import logging
import sys

import structlog


def setup_logging(level: str = "INFO", service_name: str = "hft_service") -> None:
    """Configure structlog + stdlib logging for JSON output.

    Call once at service startup before any other imports that log.
    """
    log_level = getattr(logging, level.upper(), logging.INFO)

    structlog.configure(
        processors=[
            structlog.contextvars.merge_contextvars,
            structlog.stdlib.add_log_level,
            structlog.stdlib.add_logger_name,
            structlog.processors.TimeStamper(fmt="iso", utc=True),
            structlog.processors.StackInfoRenderer(),
            structlog.processors.format_exc_info,
            structlog.processors.UnicodeDecoder(),
            # Bind static service context
            _add_service_name(service_name),
            structlog.processors.JSONRenderer(),
        ],
        wrapper_class=structlog.make_filtering_bound_logger(log_level),
        context_class=dict,
        logger_factory=structlog.PrintLoggerFactory(file=sys.stdout),
        cache_logger_on_first_use=True,
    )

    # Also configure stdlib root logger so third-party libs (confluent-kafka, etc.)
    # are captured at the right level.
    logging.basicConfig(
        stream=sys.stdout,
        level=log_level,
        format="%(message)s",  # structlog handles full formatting
    )


def _add_service_name(service_name: str):
    """structlog processor: inject `service` key into every log record."""

    def processor(_logger, _method: str, event_dict: dict) -> dict:
        event_dict.setdefault("service", service_name)
        return event_dict

    return processor


def get_logger(name: str = "") -> structlog.BoundLogger:
    """Convenience wrapper around ``structlog.get_logger``."""
    return structlog.get_logger(name)
