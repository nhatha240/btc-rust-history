"""Tests for hft_features — FeatureVector, validation, normalizer."""
import math

import numpy as np
import pytest

from hft_features.types import FeatureVector, FEATURE_FIELDS, N_FEATURES
from hft_features.validation import FeatureValidationError, validate, is_valid
from hft_features.normalizer import Normalizer
from hft_proto.md.types import FeatureStateMsg


# ── Fixtures ──────────────────────────────────────────────────────────────────

def make_valid_fv(**overrides) -> FeatureVector:
    defaults = dict(
        symbol="BTCUSDT",
        ts=1_700_000_000_000,
        ema_fast=45_000.0,
        ema_slow=44_500.0,
        rsi=55.0,
        macd=500.0,
        macd_signal=450.0,
        macd_hist=50.0,
        vwap=44_800.0,
    )
    defaults.update(overrides)
    return FeatureVector(**defaults)


# ── FeatureVector ─────────────────────────────────────────────────────────────

class TestFeatureVector:
    def test_to_array_shape(self):
        fv = make_valid_fv()
        arr = fv.to_array()
        assert arr.shape == (N_FEATURES,)
        assert arr.dtype == np.float64

    def test_to_array_order(self):
        fv = make_valid_fv()
        arr = fv.to_array()
        for i, fname in enumerate(FEATURE_FIELDS):
            assert arr[i] == getattr(fv, fname)

    def test_from_array_roundtrip(self):
        fv = make_valid_fv()
        arr = fv.to_array()
        fv2 = FeatureVector.from_array(fv.symbol, fv.ts, arr)
        for fname in FEATURE_FIELDS:
            assert getattr(fv, fname) == pytest.approx(getattr(fv2, fname))

    def test_from_array_wrong_shape_raises(self):
        with pytest.raises(ValueError, match="Expected array shape"):
            FeatureVector.from_array("BTCUSDT", 1, np.array([1.0, 2.0]))

    def test_proto_roundtrip(self):
        fv = make_valid_fv()
        proto = fv.to_proto()
        fv2 = FeatureVector.from_proto(proto)
        for fname in FEATURE_FIELDS:
            assert getattr(fv, fname) == pytest.approx(getattr(fv2, fname))
        assert fv2.symbol == fv.symbol
        assert fv2.ts == fv.ts

    def test_latency_ms_positive(self):
        import time
        fv = make_valid_fv(ts=int(time.time() * 1000) - 500)
        lag = fv.latency_ms()
        # allow some processing time tolerance
        assert 400 < lag < 2000


# ── FeatureStateMsg roundtrip ─────────────────────────────────────────────────

class TestFeatureStateMsg:
    def test_bytes_roundtrip(self):
        msg = FeatureStateMsg(
            symbol="ETHUSDT", ts=1_700_123_456_789,
            ema_fast=2_000.0, ema_slow=1_990.0, rsi=62.5,
            macd=10.0, macd_signal=8.0, macd_hist=2.0, vwap=1_995.0,
        )
        raw = msg.to_bytes()
        msg2 = FeatureStateMsg.from_bytes(raw)
        assert msg == msg2

    def test_json_roundtrip(self):
        msg = FeatureStateMsg(
            symbol="BTCUSDT", ts=1_700_000_000_000,
            ema_fast=45_000.0, ema_slow=44_800.0, rsi=55.0,
            macd=200.0, macd_signal=180.0, macd_hist=20.0, vwap=44_900.0,
        )
        msg2 = FeatureStateMsg.from_json(msg.to_json())
        assert msg == msg2


# ── Validation ────────────────────────────────────────────────────────────────

class TestValidation:
    def test_valid_passes(self):
        validate(make_valid_fv())  # should not raise

    def test_nan_raises(self):
        fv = make_valid_fv(rsi=float("nan"))
        with pytest.raises(FeatureValidationError, match="NaN"):
            validate(fv)

    def test_inf_raises(self):
        fv = make_valid_fv(vwap=float("inf"))
        with pytest.raises(FeatureValidationError, match="Inf"):
            validate(fv)

    def test_rsi_above_100(self):
        fv = make_valid_fv(rsi=100.1)
        with pytest.raises(FeatureValidationError, match="RSI"):
            validate(fv)

    def test_rsi_below_0(self):
        fv = make_valid_fv(rsi=-1.0)
        with pytest.raises(FeatureValidationError, match="RSI"):
            validate(fv)

    def test_vwap_zero(self):
        fv = make_valid_fv(vwap=0.0)
        with pytest.raises(FeatureValidationError, match="VWAP"):
            validate(fv)

    def test_negative_ema_fast(self):
        fv = make_valid_fv(ema_fast=-1.0)
        with pytest.raises(FeatureValidationError, match="ema_fast"):
            validate(fv)

    def test_is_valid_returns_tuple(self):
        ok, reason = is_valid(make_valid_fv())
        assert ok is True
        assert reason is None

    def test_is_valid_invalid_returns_false_with_reason(self):
        ok, reason = is_valid(make_valid_fv(rsi=999.0))
        assert ok is False
        assert reason is not None


# ── Normalizer ────────────────────────────────────────────────────────────────

class TestNormalizer:
    def _sample_data(self) -> np.ndarray:
        rng = np.random.default_rng(42)
        return rng.normal(loc=50_000.0, scale=1_000.0, size=(100, N_FEATURES))

    def test_zscore_transform_mean_zero(self):
        data = self._sample_data()
        norm = Normalizer.fit_zscore(data)
        transformed = norm.transform(data)
        assert abs(transformed.mean(axis=0)).max() < 0.05

    def test_zscore_transform_std_one(self):
        data = self._sample_data()
        norm = Normalizer.fit_zscore(data)
        transformed = norm.transform(data)
        assert abs(transformed.std(axis=0) - 1.0).max() < 0.05

    def test_inverse_transform_roundtrip(self):
        data = self._sample_data()
        norm = Normalizer.fit_zscore(data)
        recovered = norm.inverse_transform(norm.transform(data))
        np.testing.assert_allclose(recovered, data, rtol=1e-10)

    def test_minmax_range_01(self):
        data = self._sample_data()
        norm = Normalizer.fit_minmax(data)
        transformed = norm.transform(data)
        assert transformed.min() >= -1e-9
        assert transformed.max() <= 1.0 + 1e-9

    def test_from_dict_roundtrip(self):
        data = self._sample_data()
        norm = Normalizer.fit_zscore(data)
        norm2 = Normalizer.from_dict(norm.to_dict())
        np.testing.assert_array_equal(norm.param_a, norm2.param_a)
        np.testing.assert_array_equal(norm.param_b, norm2.param_b)

    def test_json_roundtrip(self, tmp_path):
        data = self._sample_data()
        norm = Normalizer.fit_zscore(data)
        path = str(tmp_path / "normalizer.json")
        norm.save_json(path)
        norm2 = Normalizer.load_json(path)
        np.testing.assert_array_almost_equal(norm.param_a, norm2.param_a)

    def test_transform_feature_vector(self):
        fv = make_valid_fv()
        rng = np.random.default_rng(0)
        data = rng.normal(loc=44_000.0, scale=500.0, size=(50, N_FEATURES))
        norm = Normalizer.fit_zscore(data)
        result = norm.transform_feature_vector(fv)
        assert result.shape == (N_FEATURES,)
