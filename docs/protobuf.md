# Protobuf Schema Guide

## Principles
- Schema-first, backward compatible changes only
- Avoid large optional nested structs in hot-path
- Always include:
    - trace_id
    - schema_version
    - event_time_ns + recv_time_ns
    - seq (when ordering matters)
    - feature_version / model_version to ensure compatibility

## Versioning
- schema_version: increments when message layout changes
- feature_version: increments when feature set meaning changes
- model_version: string tag (e.g. "lstm_v7_2026-02-01")

## Envelope Pattern (Recommended)
Define a common envelope (optional), or embed common fields in each message.

Example:
- trace_id: string
- schema_version: uint32
- event_time_ns: uint64
- recv_time_ns: uint64

## Message Definitions (Suggested)

### md/raw_ticks.proto
- TradeTickV1
- BookTopV1 (best bid/ask)
- Optional: DepthDeltaV1 (careful with size)

### md/features.proto
- FeatureVectorV1
  Include at minimum:
- last_price, mid_price, spread_bps
- ema_9, ema_21, rsi_14, vwap, macd_hist, funding_rate
- quality_flags bitmask

Quality flags bitmask (example):
- 1 << 0 : STALE_INPUT
- 1 << 1 : GAP_DETECTED
- 1 << 2 : OUT_OF_ORDER
- 1 << 3 : LOW_LIQUIDITY
- 1 << 4 : EXCHANGE_DISCONNECT

### ai/predictions.proto
- PredictionV1
  Fields:
- predicted_direction: int32 (1/-1/0)
- confidence_score: double (0..1)
- model_version: string
- feature_version: uint32

### oms/orders.proto
- OrderCommandV1
  Key requirements:
- client_order_id (unique)
- reduce_only for exits
- include SL/TP intent fields

### oms/execution_reports.proto
- ExecutionReportV1
  Key requirements:
- must map all exchange statuses
- include reject reason
- include filled quantities

## Compatibility Rules
Allowed changes:
- add new fields with new tags
- deprecate old fields (do not reuse tag numbers)
  Not allowed:
- changing tag numbers
- changing field types (e.g., int -> string) without new tag

## Code Generation
Rust:
- prost-build
- place generated code in libs/rust/hft_proto

Python:
- grpcio-tools or better: buf + python plugin
- place generated code in libs/python/hft_proto

## Schema Registry
- Use Redpanda/Kafka schema registry when possible
- Enforce compatibility: BACKWARD (recommended)
