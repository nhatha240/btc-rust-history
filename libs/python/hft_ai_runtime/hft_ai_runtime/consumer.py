"""
hft_ai_runtime.consumer — Async Kafka consumer wrapping confluent-kafka.

The consumer runs a poll loop in a ThreadPoolExecutor background thread
and feeds deserialized messages into an asyncio.Queue consumed by the
service's main async event loop.

Usage:
    async with AsyncKafkaConsumer(config, topics) as consumer:
        async for raw_bytes in consumer:
            msg = FeatureStateMsg.from_bytes(raw_bytes)
"""
from __future__ import annotations

import asyncio
import logging
from concurrent.futures import ThreadPoolExecutor
from typing import AsyncIterator, Callable, Optional

import structlog
from confluent_kafka import Consumer, KafkaError, KafkaException, Message

from .config import RuntimeConfig

logger = structlog.get_logger(__name__)


class AsyncKafkaConsumer:
    """Async wrapper around confluent-kafka Consumer.

    Runs the blocking ``poll()`` in a thread pool so it doesn't block
    the asyncio event loop.

    Args:
        config:         RuntimeConfig instance
        topics:         list of Kafka topic names to subscribe to
        queue_size:     max buffered messages (default 256)
        poll_timeout_s: confluent-kafka poll timeout in seconds (default 0.1)
    """

    def __init__(
        self,
        config: RuntimeConfig,
        topics: Optional[list[str]] = None,
        *,
        queue_size: int = 256,
        poll_timeout_s: float = 0.1,
    ) -> None:
        self._config = config
        self._topics = topics or config.kafka_input_topics
        self._queue: asyncio.Queue[Optional[bytes]] = asyncio.Queue(maxsize=queue_size)
        self._poll_timeout = poll_timeout_s
        self._consumer: Optional[Consumer] = None
        self._executor = ThreadPoolExecutor(max_workers=1, thread_name_prefix="kafka-consumer")
        self._running = False

    # ------------------------------------------------------------------
    # Context manager
    # ------------------------------------------------------------------

    async def __aenter__(self) -> "AsyncKafkaConsumer":
        await self.start()
        return self

    async def __aexit__(self, *args) -> None:
        await self.stop()

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    async def start(self) -> None:
        """Subscribe and start the background poll thread."""
        self._consumer = Consumer(self._config.kafka_consumer_conf())
        self._consumer.subscribe(self._topics)
        self._running = True
        loop = asyncio.get_running_loop()
        loop.run_in_executor(self._executor, self._poll_loop, loop)
        logger.info("consumer_started", topics=self._topics, brokers=self._config.kafka_brokers)

    async def stop(self) -> None:
        """Stop the poll loop and close the consumer."""
        self._running = False
        # Signal the async iterator to stop
        await self._queue.put(None)
        self._executor.shutdown(wait=True)
        if self._consumer is not None:
            self._consumer.close()
            self._consumer = None
        logger.info("consumer_stopped", topics=self._topics)

    def __aiter__(self) -> "AsyncKafkaConsumer":
        return self

    async def __anext__(self) -> bytes:
        """Yield raw message bytes. Raises StopAsyncIteration when stopped."""
        item = await self._queue.get()
        if item is None:
            raise StopAsyncIteration
        return item

    # ------------------------------------------------------------------
    # Background poll thread
    # ------------------------------------------------------------------

    def _poll_loop(self, loop: asyncio.AbstractEventLoop) -> None:
        """Blocking poll loop — runs in thread pool."""
        while self._running:
            try:
                msg: Optional[Message] = self._consumer.poll(self._poll_timeout)  # type: ignore[union-attr]
            except KafkaException as exc:
                logger.error("kafka_poll_error", error=str(exc))
                continue

            if msg is None:
                continue

            if msg.error():
                err = msg.error()
                if err.code() == KafkaError._PARTITION_EOF:
                    logger.debug("partition_eof", topic=msg.topic(), partition=msg.partition())
                else:
                    logger.error(
                        "kafka_message_error",
                        code=err.code(),
                        reason=err.str(),
                        topic=msg.topic(),
                    )
                continue

            payload = msg.value()
            if payload is None:
                continue

            asyncio.run_coroutine_threadsafe(self._queue.put(payload), loop)
