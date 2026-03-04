"""
hft_features — Feature vector types, validation, and normalization.

Modules:
    types       — FeatureVector dataclass
    validation  — validate fields (NaN/Inf/range checks)
    normalizer  — Z-score and min-max normalization helpers
"""

from .normalizer import Normalizer
from .types import FeatureVector
from .validation import FeatureValidationError, validate

__all__ = [
    "FeatureVector",
    "FeatureValidationError",
    "validate",
    "Normalizer",
]
