// ── Order types ───────────────────────────────────────────────────────────────

export type OrderSide = 'BUY' | 'SELL';
export type OrderType = 'MARKET' | 'LIMIT' | 'STOP_MARKET' | 'STOP_LIMIT' | 'TAKE_PROFIT' | 'TAKE_PROFIT_MARKET' | 'TRAILING_STOP_MARKET';
export type OrderStatus = 'NEW' | 'PARTIALLY_FILLED' | 'FILLED' | 'CANCELED' | 'REJECTED' | 'EXPIRED';
export type TimeInForce = 'GTC' | 'IOC' | 'FOK' | 'GTX';

export interface Order {
    id: number;
    client_order_id: string;
    exchange_order_id: number | null;
    account_id: string;
    symbol: string;
    side: OrderSide;
    type: OrderType;
    tif: TimeInForce;
    qty: string;
    price: string | null;
    stop_price: string | null;
    status: OrderStatus;
    filled_qty: string;
    avg_price: string | null;
    reduce_only: boolean;
    trace_id: string | null;
    strategy_version: string | null;
    created_at: string;
    updated_at: string;
}

// ── Order event types ─────────────────────────────────────────────────────────

export type OrderEventType =
    | 'SUBMITTED'
    | 'ACKNOWLEDGED'
    | 'PARTIALLY_FILLED'
    | 'FILLED'
    | 'CANCELED'
    | 'REJECTED'
    | 'EXPIRED'
    | 'REPLACE_REQUESTED';

export interface OrderEvent {
    id: number;
    client_order_id: string;
    event_type: OrderEventType;
    payload: Record<string, unknown>;
    event_time: string;
}

// ── Verification types (Handbook Alignment) ───────────────────────────────────
export interface RiskEvent {
    id: number;
    event_time: string;
    check_type: string;
    scope_type: string;
    scope_ref: string;
    severity: string;
    pass_flag: boolean;
    current_value?: string;
    limit_value?: string;
    action_taken?: string;
    related_order_id?: string;
    trace_id?: string;
}

export interface StratLog {
    id: number;
    strategy_version_id: string;
    symbol: string;
    event_time: string;
    log_level: string;
    event_code: string;
    message?: string;
    context_json?: any;
}

export interface StratHealth {
    id: number;
    instance_id: string;
    strategy_name: string;
    reported_at: string;
    cpu_pct?: string;
    mem_mb?: string;
    queue_lag_ms?: number;
    last_market_ts?: string;
    last_signal_ts?: string;
}

// ── Strategy Management ───────────────────────────────────────────────────

export type StrategyStatus = 'RUNNING' | 'PAUSED' | 'HALTED' | 'ERROR';
export type StrategyMode = 'LIVE' | 'PAPER' | 'SHADOW';

export interface Strategy {
    id: string;
    strategy_name: string;
    version: string;
    status: StrategyStatus;
    mode: StrategyMode;
    config_json: any;
    description: string | null;
    created_at: string;
    updated_at: string;
}

export interface StrategyInstance {
    id: string;
    strategy_id: string;
    instance_id: string;
    last_heartbeat: string;
    is_active: boolean;
    metadata: any;
}

export interface StrategyConfigAudit {
    id: number;
    strategy_id: string;
    changed_by: string;
    change_reason: string | null;
    old_config: any;
    new_config: any;
    created_at: string;
}

// ── Filters ───────────────────────────────────────────────────────────────────

export interface OrderFilter {
    symbol?: string;
    status?: OrderStatus;
    limit?: number;
    offset?: number;
}

export interface Signal {
    symbol: string;
    ts: number;
    side: string;
    strategy_name: string;
    price: number;
    confidence: number;
    reason: string;
}
// ── Market Data ──────────────────────────────────────────────────────────────

export interface SymbolHealth {
    symbol: string;
    last_msg_ts: number;
    msg_rate: number;
    latency_ms: number;
}

export interface VenueHealth {
    venue: string;
    reconnects: number;
    symbols: SymbolHealth[];
}

// ── Positions ─────────────────────────────────────────────────────────────────

export type PositionSide = 'LONG' | 'SHORT';

export interface Position {
    id: number;
    account_id: string;
    symbol: string;
    side: PositionSide;
    qty: string;
    entry_price: string | null;
    unrealized_pnl: string;
    realized_pnl: string;
    leverage: number;
    margin_type: string;
    liquidation_price: string | null;
    snapshot_time: string;
}
