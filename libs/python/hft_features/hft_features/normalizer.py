"""
hft_features.normalizer — Feature normalization helpers.

Provides Z-score normalization with fit/transform semantics,
and min-max scaling, matching what sklearn's StandardScaler / MinMaxScaler does
but with numpy only (no sklearn dependency).

Typical usage:
    norm = Normalizer.from_stats(means, stds)
    arr_norm = norm.transform(fv.to_array())

The Normalizer is serializable to/from a dict so it can be stored
alongside model artifacts.
"""
from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Literal

import numpy as np

from .types import FEATURE_FIELDS, N_FEATURES, FeatureVector

ScalerKind = Literal["zscore", "minmax"]


@dataclass
class Normalizer:
    """Stateful feature normalizer.

    Supports:
      - Z-score (StandardScaler): (x - mean) / std
      - Min-max: (x - min) / (max - min)
    """

    kind: ScalerKind
    param_a: np.ndarray  # mean (zscore) or min (minmax)
    param_b: np.ndarray  # std  (zscore) or max-min range (minmax)
    feature_names: tuple[str, ...] = FEATURE_FIELDS

    # ------------------------------------------------------------------
    # Factory methods
    # ------------------------------------------------------------------

    @classmethod
    def from_stats(
        cls,
        means: np.ndarray,
        stds: np.ndarray,
        *,
        epsilon: float = 1e-8,
    ) -> "Normalizer":
        """Create a Z-score normalizer from pre-computed statistics."""
        stds = np.where(stds < epsilon, epsilon, stds)
        return cls(kind="zscore", param_a=means.copy(), param_b=stds.copy())

    @classmethod
    def from_minmax(cls, mins: np.ndarray, maxs: np.ndarray, *, epsilon: float = 1e-8) -> "Normalizer":
        """Create a min-max normalizer from pre-computed min/max."""
        ranges = maxs - mins
        ranges = np.where(ranges < epsilon, epsilon, ranges)
        return cls(kind="minmax", param_a=mins.copy(), param_b=ranges.copy())

    @classmethod
    def fit_zscore(cls, data: np.ndarray) -> "Normalizer":
        """Fit from a (N, n_features) data matrix."""
        return cls.from_stats(data.mean(axis=0), data.std(axis=0))

    @classmethod
    def fit_minmax(cls, data: np.ndarray) -> "Normalizer":
        """Fit from a (N, n_features) data matrix."""
        return cls.from_minmax(data.min(axis=0), data.max(axis=0))

    # ------------------------------------------------------------------
    # Transform
    # ------------------------------------------------------------------

    def transform(self, arr: np.ndarray) -> np.ndarray:
        """Normalize a 1D or 2D array of features."""
        return (arr - self.param_a) / self.param_b

    def inverse_transform(self, arr: np.ndarray) -> np.ndarray:
        """Reverse normalization."""
        return (arr * self.param_b) + self.param_a

    def transform_feature_vector(self, fv: FeatureVector) -> np.ndarray:
        """Convenience: normalize a FeatureVector directly -> 1D array."""
        return self.transform(fv.to_array())

    # ------------------------------------------------------------------
    # Serialisation
    # ------------------------------------------------------------------

    def to_dict(self) -> dict:
        return {
            "kind": self.kind,
            "param_a": self.param_a.tolist(),
            "param_b": self.param_b.tolist(),
            "feature_names": list(self.feature_names),
        }

    @classmethod
    def from_dict(cls, d: dict) -> "Normalizer":
        return cls(
            kind=d["kind"],
            param_a=np.array(d["param_a"]),
            param_b=np.array(d["param_b"]),
            feature_names=tuple(d["feature_names"]),
        )

    def save_json(self, path: str) -> None:
        with open(path, "w") as f:
            json.dump(self.to_dict(), f, indent=2)

    @classmethod
    def load_json(cls, path: str) -> "Normalizer":
        with open(path) as f:
            return cls.from_dict(json.load(f))
