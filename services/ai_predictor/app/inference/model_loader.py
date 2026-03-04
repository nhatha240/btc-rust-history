"""
app.inference.model_loader — Load and cache the sklearn model artifact.

Provides a singleton ``ModelLoader`` that loads the model once from disk
and serves repeated inference requests without re-loading.

Expected artifact layout under MODEL_PATH:
    artifacts/model.joblib       — scikit-learn Pipeline or estimator
    artifacts/normalizer.json    — Normalizer serialized by hft_features.Normalizer
"""
from __future__ import annotations

import threading
from typing import Optional

import joblib
import structlog

from hft_features.normalizer import Normalizer

logger = structlog.get_logger(__name__)
_lock = threading.Lock()


class ModelLoader:
    """Thread-safe singleton model loader.

    Usage::

        loader = ModelLoader.instance()
        loader.load(model_path, normalizer_path)
        model = loader.model
        normalizer = loader.normalizer
    """

    _instance: Optional["ModelLoader"] = None

    def __init__(self) -> None:
        self._model = None
        self._normalizer: Optional[Normalizer] = None
        self._model_version: str = "unknown"

    # ------------------------------------------------------------------
    # Singleton
    # ------------------------------------------------------------------

    @classmethod
    def instance(cls) -> "ModelLoader":
        """Return the process-wide singleton."""
        with _lock:
            if cls._instance is None:
                cls._instance = cls()
        return cls._instance

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def load(self, model_path: str, normalizer_path: str, model_version: str = "unknown") -> None:
        """Load model and normalizer from disk.  Safe to call multiple times
        (re-loads atomically, so inference never sees a half-loaded state).
        """
        logger.info("loading_model", model_path=model_path, normalizer_path=normalizer_path)
        model = joblib.load(model_path)
        normalizer = Normalizer.load_json(normalizer_path)
        with _lock:
            self._model = model
            self._normalizer = normalizer
            self._model_version = model_version
        logger.info("model_loaded", model_version=model_version)

    @property
    def model(self):
        """The loaded sklearn estimator. Raises if not yet loaded."""
        with _lock:
            if self._model is None:
                raise RuntimeError("Model not loaded — call ModelLoader.instance().load() first")
            return self._model

    @property
    def normalizer(self) -> Normalizer:
        """The loaded Normalizer. Raises if not yet loaded."""
        with _lock:
            if self._normalizer is None:
                raise RuntimeError("Normalizer not loaded — call ModelLoader.instance().load() first")
            return self._normalizer

    @property
    def model_version(self) -> str:
        return self._model_version

    @property
    def is_loaded(self) -> bool:
        with _lock:
            return self._model is not None
