# Topics & MQ Configuration

## MQ Choice
- Redpanda/Kafka
- Protobuf (or FlatBuffers) payloads
- Avoid JSON in hot-path topics

## Naming Conventions
- md.* : market data
- ai.* : inference outputs
- orders.* : OMS loop
- control.* : config/kill-switch
- system.* : heartbeats/ops
- dlq.* : dead-letter queues

## Data Plane Topics (Hot Path)

### 1) md.raw.trades
Purpose: AggTrade stream for feature calculation  
Producer: MarketData_Ingestor  
Consumers: Feature_Engine  
Key: symbol  
Retention: 1 hour

Payload (example fields):
- symbol, event_time_ns, recv_time_ns, seq
- price, quantity, side

### 2) md.raw.book
Purpose: best bid/ask or book deltas  
Producer: MarketData_Ingestor  
Consumers: Feature_Engine  
Key: symbol  
Retention: 1 hour

Payload:
- symbol, event_time_ns, recv_time_ns, seq
- best_bid_px, best_bid_qty, best_ask_px, best_ask_qty
- optional: imbalance

### 3) md.features.live
Purpose: Live feature vectors for AI + Strategy  
Producer: Feature_Engine  
Consumers: AI_Predictor, Strategy_Engine  
Key: symbol  
Retention: 1 hour

Payload:
- symbol, event_time_ns, recv_time_ns, seq
- last_price, mid_price, spread_bps
- EMA_9, EMA_21, RSI_14, VWAP, MACD_Hist, FundingRate
- feature_version, schema_version, trace_id
- quality_flags

### 4) ai.predictions.signals
Purpose: Model inference outputs  
Producer: AI_Predictor  
Consumers: Strategy_Engine  
Key: symbol  
Retention: 24 hours

Payload:
- symbol, event_time_ns, recv_time_ns
- predicted_direction (1/-1/0)
- confidence_score (0..1)
- model_version, feature_version, schema_version, trace_id
- optional note (short)

## OMS Loop Topics (Required)

### 5) orders.commands
Purpose: Strategy emits actionable orders  
Producer: Strategy_Engine  
Consumers: Execution_Router  
Key: account_id:symbol (or account_id)  
Retention: 7-14 days (debug trace)

Payload:
- account_id, symbol
- client_order_id (idempotency key)
- side, type, qty, limit_price
- reduce_only
- stop_loss_price, take_profit_price
- decision_reason, trace_id, decision_time_ns

### 6) orders.execution_reports
Purpose: Execution lifecycle feedback  
Producer: Execution_Router  
Consumers: Strategy_Engine, DB persister (optional)  
Key: account_id:symbol  
Retention: 7-14 days

Payload:
- client_order_id, exchange_order_id
- status transitions: ACK/PARTIAL/FILLED/REJECT/CANCEL
- filled_qty, avg_fill_price, fee
- reject_reason (if any)
- event_time_ns, recv_time_ns, trace_id

### 7) positions.snapshots (optional)
Purpose: periodic position snapshots to reconcile Strategy state  
Producer: Execution_Router  
Consumers: Strategy_Engine, DB  
Key: account_id:symbol  
Retention: 7-30 days

## Control/Operational Topics

### 8) control.config_updates
Purpose: dynamic strategy/risk config pushes  
Producer: API Gateway / config service  
Consumers: Strategy_Engine, Execution_Router  
Key: account_id or bot_id  
Retention: 30-90 days

### 9) control.kill_switch
Purpose: emergency stop trading  
Producer: API Gateway  
Consumers: Strategy_Engine, Execution_Router  
Retention: 30-90 days

### 10) system.heartbeats
Purpose: service liveness and build/version  
Retention: 24 hours

### 11) dlq.*
Purpose: decode/schema mismatch, poison messages  
Retention: 7 days

## Recommended Kafka Settings (per topic class)

### Hot-path (md.* / ai.*)
- acks=all (or 1 if ultra-low-latency and acceptable loss)
- compression: lz4 (fast)
- min.insync.replicas: 2 (prod)
- partitions: scale with symbols (start 12-48)

### OMS (orders.*)
- acks=all
- idempotent producers enabled (if supported)
- partitions: by account_id (avoid splitting order state too much)
- retention longer (7-14 days)

## Consumer Groups
- cg.feature_engine
- cg.ai_predictor
- cg.strategy_engine
- cg.execution_router
- cg.db_persister (optional)
