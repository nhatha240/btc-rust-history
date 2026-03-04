"""
app.consumers.features_consumer — Kafka consumer for TOPIC_FEATURE_STATE.

Subscribes to TOPIC_FEATURE_STATE, deserializes bytes into FeatureVector,
validates each vector, and puts valid ones onto an asyncio queue for
the inference runner to pick up.
"""
from __future__ import annotations

import asyncio

import structlog

from hft_ai_runtime.consumer import AsyncKafkaConsumer
from hft_features.types import FeatureVector
from hft_features.validation import FeatureValidationError, validate
from hft_proto.md.types import FeatureStateMsg

from app.config import AiPredictorConfig
from app.metrics import get_ai_metrics

logger = structlog.get_logger(__name__)


async def run_features_consumer(
    config: AiPredictorConfig,
    inference_queue: asyncio.Queue[FeatureVector],
) -> None:
    """Consume TOPIC_FEATURE_STATE and push valid FeatureVectors to inference_queue.

    This coroutine runs indefinitely until the task is cancelled.
    """
    metrics = get_ai_metrics(config.service_name)
    topic = config.kafka_input_topics[0] if config.kafka_input_topics else "TOPIC_FEATURE_STATE"

    async with AsyncKafkaConsumer(config, topics=[topic]) as consumer:
        async for raw in consumer:
            try:
                proto_msg = FeatureStateMsg.from_bytes(raw)
            except Exception as exc:
                logger.error(
                    "deserialize_error",
                    topic=topic,
                    error=str(exc),
                )
                metrics.inc_consumer_error(topic)
                continue

            fv = FeatureVector.from_proto(proto_msg)
            metrics.inc_consumed(topic)

            try:
                validate(fv)
            except FeatureValidationError as exc:
                logger.warning(
                    "feature_validation_failed",
                    symbol=fv.symbol,
                    ts=fv.ts,
                    reason=exc.reason,
                )
                metrics.inc_validation_error(exc.reason)
                continue

            await inference_queue.put(fv)
            logger.debug("feature_enqueued", symbol=fv.symbol, ts=fv.ts)
