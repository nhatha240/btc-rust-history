# Topics & MQ Configuration

## MQ Choice
- Redpanda/Kafka
- Protobuf (or FlatBuffers) payloads
- Avoid JSON in hot-path topics

## Naming Conventions (P0)
- TOPIC_CANDLES_1M : market data OHLCV
- TOPIC_FEATURE_STATE : live calculated features
- TOPIC_SIGNALS : raw AI/strategy signals
- TOPIC_ORDERS : OMS loop commands
- TOPIC_ORDERS_APPROVED : post-risk approval
- TOPIC_FILLS : exchange execution reports
- TOPIC_MC_SNAPSHOT : market conditions/orderbook snapshots

## Data Plane Topics (Hot Path)

### 1) TOPIC_CANDLES_1M
Purpose: 1m candles for feature calculation  
Producer: Ingestor  

### 2) TOPIC_FEATURE_STATE
Purpose: Live feature vectors for AI + Strategy  
Producer: Feature_Engine  

### 3) TOPIC_SIGNALS
Purpose: AI Predictor outputs  
Producer: AI_Predictor  

## OMS Loop Topics (Required)

### 4) TOPIC_ORDERS
Purpose: Strategy emits actionable orders  
Producer: Strategy_Engine  

### 5) TOPIC_ORDERS_APPROVED
Purpose: Risk Guard approved orders  
Producer: Risk_Guard  

### 6) TOPIC_FILLS
Purpose: Execution lifecycle feedback (fills)  
Producer: Order_Executor / Paper_Trader  

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
