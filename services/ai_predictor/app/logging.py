"""
app.logging — Logging setup for ai_predictor.

Delegates to hft_ai_runtime.logging.setup_logging().
Call once at the top of app.main before any other imports.
"""
from hft_ai_runtime.logging import get_logger, setup_logging

__all__ = ["setup_logging", "get_logger"]
