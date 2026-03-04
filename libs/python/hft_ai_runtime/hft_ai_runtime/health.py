"""
hft_ai_runtime.health — Lightweight aiohttp health-check endpoint.

Starts a minimal HTTP server on HEALTH_PORT (default 8090).

Endpoints:
  GET /health  → 200 {"status": "ok", "service": "<name>", "uptime_s": 42}
  GET /ready   → 200 / 503 depending on readiness probe callback

Usage:
    server = HealthServer(config)
    server.set_live()     # mark liveness OK
    server.set_ready()    # mark readiness OK
    await server.start()
    ...
    await server.stop()
"""
from __future__ import annotations

import asyncio
import time
from typing import Optional

import structlog
from aiohttp import web

logger = structlog.get_logger(__name__)


class HealthServer:
    """Minimal aiohttp server exposing /health and /ready endpoints.

    Args:
        service_name: returned in health JSON response
        port:         TCP port to listen on (default 8090)
        host:         bind address (default 0.0.0.0)
    """

    def __init__(
        self,
        service_name: str,
        port: int = 8090,
        host: str = "0.0.0.0",
    ) -> None:
        self._service_name = service_name
        self._port = port
        self._host = host
        self._started_at = time.time()
        self._live = False
        self._ready = False
        self._runner: Optional[web.AppRunner] = None

    # ------------------------------------------------------------------
    # Probes
    # ------------------------------------------------------------------

    def set_live(self, live: bool = True) -> None:
        """Mark the liveness probe as OK (called after startup)."""
        self._live = live

    def set_ready(self, ready: bool = True) -> None:
        """Mark the readiness probe as OK (called after Kafka connection)."""
        self._ready = ready

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    async def start(self) -> None:
        """Start the HTTP server in the background."""
        app = web.Application()
        app.router.add_get("/health", self._handle_health)
        app.router.add_get("/ready", self._handle_ready)
        app.router.add_get("/", self._handle_health)

        self._runner = web.AppRunner(app, access_log=None)
        await self._runner.setup()
        site = web.TCPSite(self._runner, self._host, self._port)
        await site.start()
        logger.info("health_server_started", host=self._host, port=self._port)

    async def stop(self) -> None:
        """Gracefully shut down the HTTP server."""
        if self._runner is not None:
            await self._runner.cleanup()
            self._runner = None
        logger.info("health_server_stopped")

    # ------------------------------------------------------------------
    # Route handlers
    # ------------------------------------------------------------------

    async def _handle_health(self, _request: web.Request) -> web.Response:
        uptime = int(time.time() - self._started_at)
        return web.json_response(
            {
                "status": "ok" if self._live else "starting",
                "service": self._service_name,
                "uptime_s": uptime,
            },
            status=200 if self._live else 503,
        )

    async def _handle_ready(self, _request: web.Request) -> web.Response:
        if self._ready:
            return web.json_response({"ready": True})
        return web.json_response({"ready": False}, status=503)
