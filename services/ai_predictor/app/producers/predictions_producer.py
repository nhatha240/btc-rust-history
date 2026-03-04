"""
app.producers.predictions_producer — Publish AiPredictionMsg to TOPIC_SIGNALS.

Reads from an output queue populated by the inference runner and
produces serialized AiPredictionMsg bytes to Kafka.
"""
from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING

import structlog

from hft_proto.ai.types import AiPredictionMsg
from hft_ai_runtime.producer import AsyncKafkaProducer

from app.metrics import get_ai_metrics

if TYPE_CHECKING:
    from app.config import AiPredictorConfig

logger = structlog.get_logger(__name__)


async def run_predictions_producer(
    config: "AiPredictorConfig",
    output_queue: asyncio.Queue[AiPredictionMsg],
) -> None:
    """Consume AiPredictionMsg from output_queue and produce to Kafka.

    Runs indefinitely until the task is cancelled.
    """
    metrics = get_ai_metrics(config.service_name)
    topic = config.kafka_output_topic

    async with AsyncKafkaProducer(config) as producer:
        while True:
            msg: AiPredictionMsg = await output_queue.get()
            try:
                await producer.produce(
                    topic=topic,
                    value=msg.to_bytes(),
                    key=msg.symbol.encode(),
                )
                metrics.inc_produced(topic)
                logger.info(
                    "prediction_published",
                    symbol=msg.symbol,
                    ts=msg.ts,
                    direction=msg.direction.name,
                    confidence=f"{msg.confidence:.4f}",
                    topic=topic,
                )
            except Exception as exc:
                logger.error(
                    "produce_error",
                    symbol=msg.symbol,
                    topic=topic,
                    error=str(exc),
                )
                metrics.inc_producer_error(topic)
            finally:
                output_queue.task_done()
