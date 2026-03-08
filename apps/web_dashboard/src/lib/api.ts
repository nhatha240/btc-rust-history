import { Order, OrderEvent, OrderFilter } from './types';

const API_BASE = process.env.NEXT_PUBLIC_API_URL ?? '/api';

// ── ── Mock client-side fixtures (used when Rust api_gateway is not running) ──

const MOCK_ORDERS: Order[] = [
    {
        id: 1, client_order_id: '00000000-0000-0000-0000-000000000001',
        exchange_order_id: 1000001, account_id: 'main_account',
        symbol: 'BTCUSDT', side: 'BUY', type: 'LIMIT', tif: 'GTC',
        qty: '0.05', price: '62000.00', stop_price: null,
        status: 'FILLED', filled_qty: '0.05', avg_price: '62005.50',
        reduce_only: false, trace_id: null, strategy_version: 'v2.1.0',
        created_at: new Date(Date.now() - 2 * 3600_000).toISOString(),
        updated_at: new Date(Date.now() - 30 * 60_000).toISOString(),
    },
    {
        id: 2, client_order_id: '00000000-0000-0000-0000-000000000002',
        exchange_order_id: 1000002, account_id: 'main_account',
        symbol: 'ETHUSDT', side: 'SELL', type: 'LIMIT', tif: 'GTC',
        qty: '1.20', price: '3100.00', stop_price: null,
        status: 'CANCELED', filled_qty: '0.00', avg_price: null,
        reduce_only: false, trace_id: null, strategy_version: 'v2.1.0',
        created_at: new Date(Date.now() - 3 * 3600_000).toISOString(),
        updated_at: new Date(Date.now() - 2 * 3600_000).toISOString(),
    },
    {
        id: 3, client_order_id: '00000000-0000-0000-0000-000000000003',
        exchange_order_id: 1000003, account_id: 'main_account',
        symbol: 'BTCUSDT', side: 'BUY', type: 'LIMIT', tif: 'GTC',
        qty: '0.05', price: '61500.00', stop_price: null,
        status: 'PARTIALLY_FILLED', filled_qty: '0.02', avg_price: null,
        reduce_only: false, trace_id: null, strategy_version: 'v2.1.0',
        created_at: new Date(Date.now() - 4 * 3600_000).toISOString(),
        updated_at: new Date(Date.now() - 50 * 60_000).toISOString(),
    },
    {
        id: 4, client_order_id: '00000000-0000-0000-0000-000000000004',
        exchange_order_id: null, account_id: 'main_account',
        symbol: 'SOLUSDT', side: 'BUY', type: 'MARKET', tif: 'IOC',
        qty: '10.00', price: null, stop_price: null,
        status: 'REJECTED', filled_qty: '0.00', avg_price: null,
        reduce_only: false, trace_id: null, strategy_version: 'v2.0.9',
        created_at: new Date(Date.now() - 6 * 3600_000).toISOString(),
        updated_at: new Date(Date.now() - 6 * 3600_000).toISOString(),
    },
    {
        id: 5, client_order_id: '00000000-0000-0000-0000-000000000005',
        exchange_order_id: 1000005, account_id: 'main_account',
        symbol: 'ETHUSDT', side: 'BUY', type: 'LIMIT', tif: 'GTC',
        qty: '2.00', price: '3050.00', stop_price: null,
        status: 'NEW', filled_qty: '0.00', avg_price: null,
        reduce_only: false, trace_id: null, strategy_version: 'v2.1.0',
        created_at: new Date(Date.now() - 10 * 60_000).toISOString(),
        updated_at: new Date(Date.now() - 10 * 60_000).toISOString(),
    },
];

const MOCK_EVENTS: Record<string, OrderEvent[]> = {
    '00000000-0000-0000-0000-000000000001': [
        { id: 1, client_order_id: '00000000-0000-0000-0000-000000000001', event_type: 'SUBMITTED', payload: { action: 'submit', qty: '0.05', price: '62000.00' }, event_time: new Date(Date.now() - 2 * 3600_000).toISOString() },
        { id: 2, client_order_id: '00000000-0000-0000-0000-000000000001', event_type: 'ACKNOWLEDGED', payload: { exchange_order_id: 1000001, latency_ms: 47 }, event_time: new Date(Date.now() - 2 * 3600_000 + 1000).toISOString() },
        { id: 3, client_order_id: '00000000-0000-0000-0000-000000000001', event_type: 'PARTIALLY_FILLED', payload: { filled_qty: '0.02', avg_price: '61998.50', trade_id: 9001 }, event_time: new Date(Date.now() - 2 * 3600_000 + 30_000).toISOString() },
        { id: 4, client_order_id: '00000000-0000-0000-0000-000000000001', event_type: 'PARTIALLY_FILLED', payload: { filled_qty: '0.03', avg_price: '62010.00', trade_id: 9002 }, event_time: new Date(Date.now() - 2 * 3600_000 + 90_000).toISOString() },
        { id: 5, client_order_id: '00000000-0000-0000-0000-000000000001', event_type: 'FILLED', payload: { filled_qty: '0.05', avg_price: '62005.50', commission: '0.000025 BTC' }, event_time: new Date(Date.now() - 2 * 3600_000 + 300_000).toISOString() },
    ],
    '00000000-0000-0000-0000-000000000002': [
        { id: 1, client_order_id: '00000000-0000-0000-0000-000000000002', event_type: 'SUBMITTED', payload: { action: 'submit', qty: '1.20', price: '3100.00' }, event_time: new Date(Date.now() - 3 * 3600_000).toISOString() },
        { id: 2, client_order_id: '00000000-0000-0000-0000-000000000002', event_type: 'ACKNOWLEDGED', payload: { exchange_order_id: 1000002, latency_ms: 52 }, event_time: new Date(Date.now() - 3 * 3600_000 + 1000).toISOString() },
        { id: 3, client_order_id: '00000000-0000-0000-0000-000000000002', event_type: 'CANCELED', payload: { reason: 'USER_CANCEL' }, event_time: new Date(Date.now() - 2 * 3600_000).toISOString() },
    ],
};

// ── ── API functions ───────────────────────────────────────────────────────────

async function fetchJson<T>(url: string): Promise<T> {
    const res = await fetch(url, { next: { revalidate: 10 } });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json() as Promise<T>;
}

export async function fetchOrders(filter: OrderFilter = {}): Promise<Order[]> {
    const params = new URLSearchParams();
    if (filter.symbol) params.set('symbol', filter.symbol);
    if (filter.status) params.set('status', filter.status);
    if (filter.limit) params.set('limit', String(filter.limit));
    if (filter.offset) params.set('offset', String(filter.offset));

    const qs = params.toString();
    const url = `${API_BASE}/orders${qs ? `?${qs}` : ''}`;

    try {
        return await fetchJson<Order[]>(url);
    } catch {
        // Fallback to mock when API is unavailable
        console.warn('[api] Using mock orders');
        let orders = MOCK_ORDERS;
        if (filter.symbol) orders = orders.filter(o => o.symbol === filter.symbol);
        if (filter.status) orders = orders.filter(o => o.status === filter.status);
        return orders;
    }
}

export async function fetchOrder(id: string): Promise<Order | null> {
    try {
        return await fetchJson<Order>(`${API_BASE}/orders/${id}`);
    } catch {
        console.warn('[api] Using mock order');
        return MOCK_ORDERS.find(o => o.client_order_id === id) ?? null;
    }
}

export async function fetchOrderEvents(id: string): Promise<OrderEvent[]> {
    try {
        return await fetchJson<OrderEvent[]>(`${API_BASE}/orders/${id}/events`);
    } catch {
        console.warn('[api] Using mock events');
        return MOCK_EVENTS[id] ?? [];
    }
}

export async function fetchRiskEvents(): Promise<import('./types').RiskEvent[]> {
    try {
        return await fetchJson<import('./types').RiskEvent[]>(`${API_BASE}/verification/risk_events`);
    } catch {
        return [];
    }
}

export async function fetchStratLogs(): Promise<import('./types').StratLog[]> {
    try {
        return await fetchJson<import('./types').StratLog[]>(`${API_BASE}/verification/strat_logs`);
    } catch {
        return [];
    }
}

export async function fetchStratHealth(): Promise<import('./types').StratHealth[]> {
    try {
        return await fetchJson<import('./types').StratHealth[]>(`${API_BASE}/verification/strat_health`);
    } catch {
        return [];
    }
}
