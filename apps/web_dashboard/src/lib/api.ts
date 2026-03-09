import { VenueHealth } from './types';

export async function fetchMdHealth(): Promise<VenueHealth[]> {
  const response = await fetch('/api/md/health');
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
  const url = new URL('/api/signals', window.location.origin);

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
  const url = new URL('/api/positions', window.location.origin);
  if (accountId) {
    url.searchParams.append('account_id', accountId);
  }
  const response = await fetch(url);
  if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
  return response.json();
}

export async function closePosition(symbol: string, accountId?: string): Promise<{ status: string; symbol: string }> {
  const url = new URL(`/api/positions/${symbol}/close`, window.location.origin);
  if (accountId) {
    url.searchParams.append('account_id', accountId);
  }
  const response = await fetch(url, { method: 'POST' });
  if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
  return response.json();
}

export async function partialClosePosition(symbol: string, qty: string, accountId?: string): Promise<{ status: string; symbol: string; qty: string }> {
  const url = new URL(`/api/positions/${symbol}/partial_close`, window.location.origin);
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