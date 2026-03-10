import { VenueHealth } from './types';

const getBaseUrl = () => {
    if (typeof window !== 'undefined') return ''; // Browser uses relative path
    return process.env.NEXT_PUBLIC_API_URL || 'http://api_gateway:8080'; // Server uses internal hostname
};

export async function fetchMdHealth(): Promise<VenueHealth[]> {
  const response = await fetch(`${getBaseUrl()}/api/md/health`);
  if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
  return response.json();
}

export interface Signal {
  signal_id: string;
  strategy_id: string;
  symbol: string;
  timeframe: string;
  side: 'LONG' | 'SHORT';
  confidence: number;
  regime: number; // 1: TREND_UP, 2: TREND_DOWN, 3: RANGE, 4: VOLATILE_PANIC
  score: number; // overall score
  features: Record<string, number>;
  reason: string;
  note: string;
  created_at: number;
  expired_at: number | null;
  signal_status: 'pending' | 'executed' | 'rejected' | 'expired' | 'blocked_by_risk';
  expected_rr: number; // risk-reward ratio
  expected_volatility: number;
  higher_timeframe_confirmation: boolean;
  threshold_decision: Record<string, number>;
  blocked_reason: string | null;
  top_contributing_features: Array<{ feature: string; weight: number }>; // top 3 contributing features
}

export async function fetchSignals(params: {
  symbol?: string;
  limit?: number;
  offset?: number;
  side?: 'LONG' | 'SHORT';
  strategy_id?: string;
  confidence_min?: number;
  regime?: number;
  status?: 'pending' | 'executed' | 'rejected' | 'expired' | 'blocked_by_risk';
  start_date?: string;
  end_date?: string;
  signal_id?: string;
} = {}): Promise<Signal[]> {
  const url = new URL(`${getBaseUrl()}/api/signals`);

  if (params.symbol) url.searchParams.append('symbol', params.symbol);
  if (params.limit) url.searchParams.append('limit', params.limit.toString());
  if (params.offset) url.searchParams.append('offset', params.offset.toString());
  if (params.side) url.searchParams.append('side', params.side);
  if (params.strategy_id) url.searchParams.append('strategy_id', params.strategy_id);
  if (params.confidence_min) url.searchParams.append('confidence_min', params.confidence_min.toString());
  if (params.regime) url.searchParams.append('regime', params.regime.toString());
  if (params.status) url.searchParams.append('status', params.status);
  if (params.start_date) url.searchParams.append('start_date', params.start_date);
  if (params.end_date) url.searchParams.append('end_date', params.end_date);
  if (params.signal_id) url.searchParams.append('signal_id', params.signal_id);

  const response = await fetch(url);

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
}

// Add this to the end of the file
export async function fetchActiveSignals(): Promise<Signal[]> {
  return fetchSignals({ limit: 100, side: 'LONG' });
}

export async function fetchShortSignals(): Promise<Signal[]> {
  return fetchSignals({ limit: 100, side: 'SHORT' });
}

export async function fetchSignalRanking(): Promise<Signal[]> {
  return fetchSignals({ limit: 20 });
}

// ── Positions API ─────────────────────────────────────────────────────────────

import { Position } from './types';

export async function fetchPositions(accountId?: string): Promise<Position[]> {
  const url = new URL(`${getBaseUrl()}/api/positions`);
  if (accountId) {
    url.searchParams.append('account_id', accountId);
  }
  const response = await fetch(url);
  if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
  return response.json();
}

export async function closePosition(symbol: string, accountId?: string): Promise<{ status: string; symbol: string }> {
  const url = new URL(`${getBaseUrl()}/api/positions/${symbol}/close`);
  if (accountId) {
    url.searchParams.append('account_id', accountId);
  }
  const response = await fetch(url, { method: 'POST' });
  if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
  return response.json();
}

export async function partialClosePosition(symbol: string, qty: string, accountId?: string): Promise<{ status: string; symbol: string; qty: string }> {
  const url = new URL(`${getBaseUrl()}/api/positions/${symbol}/partial_close`);
  const response = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      account_id: accountId,
      qty,
    }),
  });
  if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
  return response.json();
}

import {
  Order,
  OrderEvent,
  Strategy,
  ErrorLog,
  StratLog,
  RiskEventRecord,
  StrategyConfigAudit,
  StrategyConfigUpdatePayload,
  StratHealth,
  RiskEvent,
} from './types';

export async function fetchOrder(id: string): Promise<Order> {
  const url = new URL(`${getBaseUrl()}/api/orders/${id}`);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
  return response.json();
}

export async function fetchOrders(params: {
  symbol?: string;
  status?: string;
  limit?: number;
} = {}): Promise<Order[]> {
  const url = new URL(`${getBaseUrl()}/api/orders`);
  if (params.symbol) url.searchParams.append('symbol', params.symbol);
  if (params.status) url.searchParams.append('status', params.status);
  if (params.limit) url.searchParams.append('limit', params.limit.toString());

  const response = await fetch(url);
  if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
  return response.json();
}

export async function fetchOrderEvents(id: string): Promise<OrderEvent[]> {
  const url = new URL(`${getBaseUrl()}/api/orders/${id}/events`);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
  return response.json();
}

// ── Trades API ────────────────────────────────────────────────────────────────
import { Trade } from './types';

export async function fetchTrades(params: {
  symbol?: string;
  limit?: number;
  offset?: number;
} = {}): Promise<Trade[]> {
  const url = new URL(`${getBaseUrl()}/api/trades`);
  if (params.symbol) url.searchParams.append('symbol', params.symbol);
  if (params.limit) url.searchParams.append('limit', params.limit.toString());
  if (params.offset) url.searchParams.append('offset', params.offset.toString());

  const response = await fetch(url);
  if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
  return response.json();
}

export async function cancelOrder(id: string): Promise<{ status: string }> {
  const url = new URL(`${getBaseUrl()}/api/orders/${id}/cancel`);
  const response = await fetch(url, { method: 'POST' });
  if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
  return response.json();
}

export async function cancelAllOrders(params: { symbol?: string, strategyId?: string } = {}): Promise<{ status: string, cancelled_count: number }> {
  const url = new URL(`${getBaseUrl()}/api/orders/cancel_all`);
  if (params.symbol) url.searchParams.append('symbol', params.symbol);
  if (params.strategyId) url.searchParams.append('strategy_id', params.strategyId);

  const response = await fetch(url, { method: 'POST' });
  if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
  return response.json();
}

export async function fetchSystemLogs(params: { service?: string, severity?: string } = {}): Promise<ErrorLog[]> {
  const url = new URL(`${getBaseUrl()}/api/logs/system`);
  if (params.service) url.searchParams.append('service', params.service);
  if (params.severity) url.searchParams.append('severity', params.severity);
  const response = await fetch(url);
  return response.json();
}

export async function fetchStrategyLogs(params: { strategyId?: string, symbol?: string } = {}): Promise<StratLog[]> {
  const url = new URL(`${getBaseUrl()}/api/logs/strategy`);
  if (params.strategyId) url.searchParams.append('strategy_id', params.strategyId);
  if (params.symbol) url.searchParams.append('symbol', params.symbol);
  const response = await fetch(url);
  return response.json();
}

export async function fetchRiskLogs(params: { accountId?: string, eventType?: string } = {}): Promise<RiskEventRecord[]> {
  const url = new URL(`${getBaseUrl()}/api/logs/risk`);
  if (params.accountId) url.searchParams.append('account_id', params.accountId);
  if (params.eventType) url.searchParams.append('event_type', params.eventType);
  const response = await fetch(url);
  return response.json();
}

export async function fetchAuditLogs(params: { strategyId?: string } = {}): Promise<StrategyConfigAudit[]> {
  const url = new URL(`${getBaseUrl()}/api/logs/audit`);
  if (params.strategyId) url.searchParams.append('strategy_id', params.strategyId);
  const response = await fetch(url);
  return response.json();
}

async function throwHttpError(response: Response): Promise<never> {
  let details = '';
  try {
    details = await response.text();
  } catch {
    details = '';
  }

  const suffix = details ? ` - ${details}` : '';
  throw new Error(`HTTP error! status: ${response.status}${suffix}`);
}

export async function fetchStrategies(): Promise<Strategy[]> {
  const response = await fetch(`${getBaseUrl()}/api/strategies`);
  if (!response.ok) await throwHttpError(response);
  return response.json();
}

export async function updateStrategyAction(id: string, action: string): Promise<boolean> {
  const response = await fetch(`${getBaseUrl()}/api/strategies/${id}/action`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ action }),
  });

  if (!response.ok) await throwHttpError(response);
  return true;
}

export async function updateStrategyConfig(
  id: string,
  payload: StrategyConfigUpdatePayload,
): Promise<boolean> {
  const response = await fetch(`${getBaseUrl()}/api/strategies/${id}/config`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      config: payload.config,
      changed_by: payload.changed_by,
      reason: payload.reason,
    }),
  });

  if (!response.ok) await throwHttpError(response);
  return true;
}

export async function fetchStrategyConfigAudit(strategyId: string): Promise<StrategyConfigAudit[]> {
  const response = await fetch(`${getBaseUrl()}/api/strategies/${strategyId}/audit`);
  if (!response.ok) await throwHttpError(response);
  return response.json();
}

export async function fetchStratHealth(): Promise<StratHealth[]> {
  const response = await fetch(`${getBaseUrl()}/api/verification/strat_health`);
  if (!response.ok) await throwHttpError(response);
  return response.json();
}

export async function fetchRiskEvents(): Promise<RiskEvent[]> {
  const response = await fetch(`${getBaseUrl()}/api/verification/risk_events`);
  if (!response.ok) await throwHttpError(response);
  return response.json();
}

export async function fetchStratLogs(): Promise<StratLog[]> {
  const response = await fetch(`${getBaseUrl()}/api/verification/strat_logs`);
  if (!response.ok) await throwHttpError(response);
  return response.json();
}
