import asyncio
import uuid
import time
import os
import psycopg2
from confluent_kafka import Producer
from hft_proto.oms.orders_pb2 import OrderCommand, OrderSide, OrderType, TimeInForce

# Config
KAFKA_BROKERS = os.getenv("KAFKA_BROKERS", "localhost:29092")
TOPIC_ORDERS = os.getenv("KAFKA_TOPIC_ORDERS", "TOPIC_ORDERS")
DATABASE_URL = os.getenv("DATABASE_URL", "postgres://trader:traderpw@localhost:5432/db_trading")

async def verify_oms_loop():
    print(f"🚀 Starting OMS Loop Verification...")
    
    # 1. Setup Kafka Producer
    p = Producer({"bootstrap.servers": KAFKA_BROKERS})
    
    # 2. Create Mock Order
    client_order_id = str(uuid.uuid4())
    order = OrderCommand(
        account_id="test-account-01",
        symbol="BTCUSDT",
        client_order_id=client_order_id,
        side=OrderSide.BUY,
        type=OrderType.MARKET,
        tif=TimeInForce.GTC,
        qty=0.01,
        price=60000.0,
        reduce_only=False,
        decision_reason="E2E Verification",
        trace_id=str(uuid.uuid4()),
        decision_time_ns=int(time.time() * 1e9)
    )
    
    # 3. Produce to TOPIC_ORDERS
    print(f"📤 Sending Order: {client_order_id}")
    p.produce(TOPIC_ORDERS, order.SerializeToString(), key=order.account_id)
    p.flush()
    
    print("⏳ Waiting for processing (3 seconds)...")
    await asyncio.sleep(3)
    
    # 4. Verify in Postgres
    print("🔍 Checking Database...")
    conn = psycopg2.connect(DATABASE_URL)
    cur = conn.cursor()
    
    cur.execute("SELECT status, exchange_order_id FROM orders WHERE client_order_id = %s", (client_order_id,))
    res = cur.fetchone()
    
    if res:
        status, ex_id = res
        print(f"✅ Order found in DB! Status: {status}, Exchange ID: {ex_id}")
        
        cur.execute("SELECT price, qty FROM trades WHERE client_order_id = %s", (client_order_id,))
        trade = cur.fetchone()
        if trade:
            print(f"✅ Trade (Fill) found in DB! Price: {trade[0]}, Qty: {trade[1]}")
        else:
            print("❌ Trade not found in DB.")
            
        cur.execute("SELECT qty FROM positions WHERE symbol = 'BTCUSDT' AND account_id = 'test-account-01'")
        pos = cur.fetchone()
        if pos:
            print(f"✅ Position updated! Current Qty: {pos[0]}")
        else:
            print("❌ Position not found.")
    else:
        print("❌ Order NOT found in DB. Check logs of risk_guard, paper_trader, or order_executor.")
        
    cur.close()
    conn.close()

if __name__ == "__main__":
    try:
        asyncio.run(verify_oms_loop())
    except Exception as e:
        print(f"💥 Verification failed: {e}")
