Plan tổng quan (P0 → P1)
P0: Chạy end-to-end và có Web Order History

Mục tiêu P0:

MQ chạy ổn + topics đúng

OMS loop chạy: signal/order → risk → executor → fills/events → DB

Web/API đọc DB hiển thị orders timeline

Observability cơ bản: health + metrics + trace_id

P1: Data-plane đầy đủ + AI predictor + analytics ClickHouse

Mục tiêu P1:

WS raw ticks → feature engine → AI predictor → strategy

Backtest/training dataset + ClickHouse analytics + dashboards nâng cao

Giai đoạn 0 — “Repo foundation” (0.5–1 ngày)
Deliverables

Monorepo structure (như cây thư mục đã chốt)

Tooling & conventions

Docker compose dev stable

Tasks chi tiết

 Chốt naming conventions:

services: ingestion, feature_state, signal_engine, risk_guard, order_executor, paper_trader, web

topics: giữ TOPIC_* (để đồng bộ compose hiện tại) hoặc chuyển domain naming (md., orders.) — P0 giữ TOPIC_* cho nhanh.

 Thiết lập workspace build:

Rust workspace (bins theo BIN=...)

Python env (nếu có, để P1)

Web (Next.js) để P0

 Chốt file .env.example cho toàn stack

 Compose:

đảm bảo “ClickHouse optional” (không block core)

sửa mismatch topic env (đã fix)

Exit criteria

docker compose up chạy được

Redpanda console lên UI

Postgres healthy

Redis healthy

Giai đoạn 1 — “Schema & Contract” (P0 bắt buộc)

Không schema/contract thì code service sẽ lệch nhau.

Deliverables

Topic contract (payload fields) — có thể là Protobuf hoặc internal binary

DB schema cho order history (Postgres)

Tasks chi tiết

 Chốt message fields tối thiểu cho P0:

Signal/Order: symbol, side, qty, price intent, strategy_version, trace_id, ts

Risk approval: approved/rejected + reason

Fill/Event: client_order_id, exchange_order_id, status, filled_qty, avg_price, fee, reject_reason, trace_id, ts

 Chốt idempotency keys:

client_order_id unique

trace_id correlation

 Chốt DB tables P0:

orders

order_events

fills (hoặc trades)

positions (optional P0, nhưng nên có)

decision_logs (optional, cực hữu ích)

 Indexing rules:

orders: (symbol, created_at desc), (status, created_at desc), unique(client_order_id)

events: (order_id, recv_time asc)

fills: (symbol, trade_time desc)

Exit criteria

Migrations chạy OK, DB có đủ bảng

Tài liệu docs/topics.md + docs/runbook.md cập nhật theo thực tế P0

Giai đoạn 2 — “OMS loop first” (quan trọng nhất để có web order history)

Mục tiêu: Tạo được dòng dữ liệu orders → events/fills → DB kể cả khi chưa có market data thật.

Deliverables

order_executor ghi DB chuẩn (orders + events + fills)

paper_trader chạy được để mô phỏng fills

risk_guard approve/reject có reason

Có “E2E demo” bằng order mock

Process flow P0 (đề xuất)

signal_engine phát tín hiệu → TOPIC_ORDERS

risk_guard nhận TOPIC_ORDERS → phát TOPIC_ORDERS_APPROVED

paper_trader nhận TOPIC_ORDERS_APPROVED → phát TOPIC_FILLS

order_executor subscribe TOPIC_FILLS (hoặc report topic) → persist Postgres:

upsert orders

insert order_events

insert fills

update positions (optional)

Nếu order_executor hiện đang đặt lệnh thật Binance, P0 bạn khóa EXCHANGE=paper hoặc chạy paper_trader trước để không rủi ro.

Tasks chi tiết theo service
A) risk_guard

 Validate input order

 Apply limits:

max notional per symbol

leverage cap

kill-switch key in Redis

 Produce “approved/rejected” decision + reason

 Metrics:

approvals count, rejects count by reason

B) paper_trader

 Simulate immediate ACK + FILL event

 Support partial fill optional (P0 có thể bỏ)

 Emit fills with trace_id + client_order_id

C) order_executor (persist)

 Consume fills/events

 Convert to DB writes:

create order if not exists

append timeline event rows

store fills

update positions snapshot

 Ensure idempotency:

unique constraint on fill_id or (client_order_id, fill_seq)

ignore duplicates

Exit criteria

Send 1 order mock → web query thấy:

order row

timeline event (ACK/FILL)

fill row

Giai đoạn 3 — “Web/API MVP” (đúng yêu cầu của bạn)

Mục tiêu: có web xem lịch sử lệnh các kiểu.

Deliverables

API Gateway (service web) query Postgres

Web UI (Next.js) hiển thị list + detail timeline

API endpoints P0

 GET /api/orders?symbol=&status=&from=&to=&limit=&cursor=

 GET /api/orders/{id} (include events + fills)

 GET /api/positions/current (optional)

 GET /api/health

Web pages P0

 /orders:

table: time, symbol, side, qty, status, trace_id

filters: symbol, status, time range

 /orders/[id]:

summary card

timeline events

fills table

Exit criteria

Demo: click vào order thấy timeline + fills

Filter hoạt động

Pagination không lag

Giai đoạn 4 — “Stability & Observability” (P0 hardening)
Deliverables

Health/readiness chuẩn

Metrics + tracing tối thiểu

Runbook vận hành

Tasks chi tiết

 Propagate trace_id xuyên pipeline (signal→risk→paper/executor→db)

 Add structured logs (JSON logs cold-path OK)

 Prometheus metrics:

consume lag (nếu đo được)

processed msg/s

reject rate

db write latency p95

 Circuit breakers:

kill switch via Redis key

rate limiting counters

cooldown

Exit criteria

Grafana panel basic hiển thị:

order rate

reject rate

service health

Kill switch bật là dừng orders mới

P1 (Sau khi P0 chạy) — Data-plane + AI + ClickHouse
P1.1 Market data WS → features

 MarketData_Ingestor WS (AggTrade + best bid/ask)

 Feature engine incremental indicators

 Emit md.features.live (hoặc reuse feature_state topic)

P1.2 AI Predictor

 Subscribe features

 Inference + publish predictions

 Strategy consumes predictions + features

P1.3 ClickHouse analytics (bật sau)

 Persister write candles/features snapshots → ClickHouse

 Web charts query ClickHouse (PnL aggregation / latency hist)