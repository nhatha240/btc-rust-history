"""
app.main — Entry point for the ai_predictor service.

Start-up sequence:
  1. Load config from env vars
  2. Set up structured JSON logging
  3. Load model + normalizer from disk
  4. Start Prometheus metrics HTTP server
  5. Start health-check HTTP server
  6. Mark service as live
  7. Start async tasks: Kafka consumer loop + inference loop + producer loop
  8. Mark service as ready
  9. Wait until SIGINT / SIGTERM

Graceful shutdown:
  - Cancel all tasks
  - Flush Kafka producer
  - Stop health server
"""
from __future__ import annotations

import asyncio
import signal
import sys
from concurrent.futures import ThreadPoolExecutor

import structlog

from hft_ai_runtime.health import HealthServer
from hft_proto.ai.types import AiPredictionMsg
from hft_features.types import FeatureVector

from app.config import AiPredictorConfig
from app.consumers.features_consumer import run_features_consumer
from app.inference.model_loader import ModelLoader
from app.inference.postprocess import build_prediction
from app.inference.runner import run_inference
from app.logging import setup_logging
from app.metrics import get_ai_metrics, register_model_info
from app.producers.predictions_producer import run_predictions_producer

logger = structlog.get_logger(__name__)

# Queue sizes
_INFERENCE_QUEUE_SIZE = 128
_OUTPUT_QUEUE_SIZE = 128


async def _inference_loop(
    config: AiPredictorConfig,
    inference_queue: asyncio.Queue[FeatureVector],
    output_queue: asyncio.Queue[AiPredictionMsg],
    executor: ThreadPoolExecutor,
) -> None:
    """Dequeue FeatureVectors, run inference in thread pool, enqueue results."""
    loop = asyncio.get_running_loop()
    while True:
        fv = await inference_queue.get()
        try:
            direction, confidence, raw_score = await loop.run_in_executor(
                executor,
                run_inference,
                fv,
                config,
            )
            prediction = build_prediction(
                fv=fv,
                direction=direction,
                confidence=confidence,
                raw_score=raw_score,
                model_version=config.model_version,
            )
            await output_queue.put(prediction)
        except Exception as exc:
            logger.error("inference_error", symbol=fv.symbol, ts=fv.ts, error=str(exc))
        finally:
            inference_queue.task_done()


async def _main() -> None:
    config = AiPredictorConfig.from_env()
    setup_logging(level=config.log_level, service_name=config.service_name)
    logger.info("starting", service=config.service_name, version=config.model_version)

    # ── Load model ───────────────────────────────────────────────────
    loader = ModelLoader.instance()
    loader.load(config.model_path, config.normalizer_path, config.model_version)

    # ── Metrics ──────────────────────────────────────────────────────
    metrics = get_ai_metrics(config.service_name)
    register_model_info(config.model_version)
    metrics.start_http_server(port=config.metrics_port)
    logger.info("metrics_server_started", port=config.metrics_port)

    # ── Health ───────────────────────────────────────────────────────
    health = HealthServer(
        service_name=config.service_name,
        port=config.health_port,
        host=config.health_host,
    )
    await health.start()
    health.set_live(True)

    # ── Queues ───────────────────────────────────────────────────────
    inference_queue: asyncio.Queue[FeatureVector] = asyncio.Queue(maxsize=_INFERENCE_QUEUE_SIZE)
    output_queue: asyncio.Queue[AiPredictionMsg] = asyncio.Queue(maxsize=_OUTPUT_QUEUE_SIZE)

    executor = ThreadPoolExecutor(max_workers=2, thread_name_prefix="inference")

    # ── Tasks ────────────────────────────────────────────────────────
    tasks = [
        asyncio.create_task(run_features_consumer(config, inference_queue), name="consumer"),
        asyncio.create_task(
            _inference_loop(config, inference_queue, output_queue, executor),
            name="inference",
        ),
        asyncio.create_task(run_predictions_producer(config, output_queue), name="producer"),
    ]

    health.set_ready(True)
    logger.info("service_ready", topics=config.kafka_input_topics, output=config.kafka_output_topic)

    # ── Wait for shutdown ────────────────────────────────────────────
    loop = asyncio.get_running_loop()
    stop_event = asyncio.Event()

    def _handle_signal():
        logger.info("shutdown_signal_received")
        stop_event.set()

    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, _handle_signal)

    await stop_event.wait()

    # ── Graceful shutdown ────────────────────────────────────────────
    logger.info("shutting_down")
    health.set_ready(False)
    for task in tasks:
        task.cancel()
    await asyncio.gather(*tasks, return_exceptions=True)
    executor.shutdown(wait=False)
    await health.stop()
    logger.info("shutdown_complete")


def run() -> None:
    """Console script entry point (see pyproject.toml [project.scripts])."""
    asyncio.run(_main())


if __name__ == "__main__":
    run()
