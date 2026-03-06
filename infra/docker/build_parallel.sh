#!/bin/bash
services=("web" "ingestion" "feature_state" "mc_snapshot" "signal_engine" "risk_guard" "order_executor" "api_gateway" "execution_router")
echo "Starting parallel builds..."
for svc in "${services[@]}"; do
    echo "Building $svc in background... Logs in build_$svc.log"
    docker compose build $svc > "build_$svc.log" 2>&1 &
done
echo "Waiting for all builds to finish..."
wait
echo "All builds finished. Check the log files."
