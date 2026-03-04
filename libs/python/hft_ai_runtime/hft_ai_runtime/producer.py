"""
hft_ai_runtime.producer — Async Kafka producer wrapping confluent-kafka.

Provides a fire-and-forget ``produce`` method and a ``flush`` for graceful
shutdown.  Delivery callbacks increment Prometheus metrics and log errors.

Usage:
    async with AsyncKafkaProducer(config) as producer:
        await producer.produce(topic, value=msg.to_bytes(), key=symbol.encode())
"""
from __future__ import annotations

import asyncio
import logging
from concurrent.futures import ThreadPoolExecutor
from typing import Optional

import structlog
from confluent_kafka import KafkaException, Producer

from .config import RuntimeConfig

logger = structlog.get_logger(__name__)


class AsyncKafkaProducer:
    """Async wrapper around confluent-kafka Producer.

    Runs ``poll()`` in a background thread to drain delivery callbacks
    without blocking the asyncio event loop.

    Args:
        config:         RuntimeConfig instance
        flush_timeout_s: how long to wait for in-flight messages on shutdown
    """

    def __init__(
        self,
        config: RuntimeConfig,
        *,
        flush_timeout_s: float = 10.0,
    ) -> None:
        self._config = config
        self._flush_timeout = flush_timeout_s
        self._producer: Optional[Producer] = None
        self._executor = ThreadPoolExecutor(max_workers=1, thread_name_prefix="kafka-producer")
        self._running = False

    # ------------------------------------------------------------------
    # Context manager
    # ------------------------------------------------------------------

    async def __aenter__(self) -> "AsyncKafkaProducer":
        await self.start()
        return self

    async def __aexit__(self, *args) -> None:
        await self.stop()

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    async def start(self) -> None:
        """Create the underlying Producer and start the poll thread."""
        self._producer = Producer(self._config.kafka_producer_conf())
        self._running = True
        loop = asyncio.get_running_loop()
        loop.run_in_executor(self._executor, self._poll_loop)
        logger.info("producer_started", brokers=self._config.kafka_brokers)

    async def stop(self) -> None:
        """Flush in-flight messages and shut down."""
        self._running = False
        if self._producer is not None:
            remaining = self._producer.flush(timeout=self._flush_timeout)
            if remaining > 0:
                logger.warning("producer_flush_timeout", remaining=remaining)
            self._producer = None
        self._executor.shutdown(wait=True)
        logger.info("producer_stopped")

    async def produce(
        self,
        topic: str,
        value: bytes,
        *,
        key: Optional[bytes] = None,
        headers: Optional[dict] = None,
    ) -> None:
        """Non-blocking produce — returns as soon as the message is enqueued.

        Delivery is confirmed asynchronously via the callback logged below.
        """
        if self._producer is None:
            raise RuntimeError("Producer is not started. Use `async with` or call start().")

        def _delivery_callback(err, msg):
            if err:
                logger.error(
                    "producer_delivery_failed",
                    topic=topic,
                    error=str(err),
                )
            else:
                logger.debug(
                    "producer_delivery_ok",
                    topic=msg.topic(),
                    partition=msg.partition(),
                    offset=msg.offset(),
                )

        loop = asyncio.get_running_loop()
        await loop.run_in_executor(
            None,
            lambda: self._producer.produce(  # type: ignore[union-attr]
                topic=topic,
                value=value,
                key=key,
                headers=headers or {},
                on_delivery=_delivery_callback,
            ),
        )

    # ------------------------------------------------------------------
    # Background poll thread (drains delivery callbacks)
    # ------------------------------------------------------------------

    def _poll_loop(self) -> None:
        """Poll in background thread to trigger delivery callbacks."""
        import time

        while self._running:
            if self._producer is not None:
                self._producer.poll(0.05)
            else:
                time.sleep(0.05)
