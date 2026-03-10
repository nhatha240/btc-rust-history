#!/bin/bash

services=(
  "web_dashboard"
  "marketdata_ingestor"
  "feature_state"
  "mc_snapshot"
  "signal_engine"
  "risk_guard"
  "order_executor"
  "api_gateway"
  "execution_router"
  "planner"
  "feature_engine"
)

echo "Starting builds in batches of 2..."

for ((i=0; i<${#services[@]}; i+=2)); do
  pids=()
 echo "Building $svc in background... Logs in build_${svc}.log"
  for ((j=0; j<2 && i+j<${#services[@]}; j++)); do
    svc="${services[i+j]}"
    echo "Building $svc in background... Logs in build_${svc}.log"
    docker compose build "$svc" > "build_${svc}.log" 2>&1 &
    pids+=("$!")
  done

  for pid in "${pids[@]}"; do
    wait "$pid" || exit 1
  done
done

echo "All builds finished. Check the log files."