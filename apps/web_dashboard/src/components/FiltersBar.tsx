'use client';

import { OrderStatus } from '@/lib/types';

const SYMBOLS = ['', 'BTCUSDT', 'ETHUSDT', 'SOLUSDT', 'BNBUSDT'];
const STATUSES: Array<{ value: OrderStatus | ''; label: string }> = [
    { value: '', label: 'All Statuses' },
    { value: 'NEW', label: 'New' },
    { value: 'PARTIALLY_FILLED', label: 'Partially Filled' },
    { value: 'FILLED', label: 'Filled' },
    { value: 'CANCELED', label: 'Canceled' },
    { value: 'REJECTED', label: 'Rejected' },
    { value: 'EXPIRED', label: 'Expired' },
];

interface Props {
    symbol: string;
    status: string;
    onSymbolChange: (v: string) => void;
    onStatusChange: (v: string) => void;
    onRefresh?: () => void;
    isLoading?: boolean;
}

export function FiltersBar({ symbol, status, onSymbolChange, onStatusChange, onRefresh, isLoading }: Props) {
    const selectCls = `
    bg-slate-800 border border-white/10 text-slate-300 text-sm rounded-lg px-3 py-2
    focus:outline-none focus:border-indigo-500 transition-colors
  `;

    return (
        <div className="flex flex-wrap items-center gap-3">
            {/* Symbol picker */}
            <select value={symbol} onChange={e => onSymbolChange(e.target.value)} className={selectCls}>
                {SYMBOLS.map(s => (
                    <option key={s} value={s}>{s || 'All Symbols'}</option>
                ))}
            </select>

            {/* Status picker */}
            <select value={status} onChange={e => onStatusChange(e.target.value)} className={selectCls}>
                {STATUSES.map(s => (
                    <option key={s.value} value={s.value}>{s.label}</option>
                ))}
            </select>

            {/* Refresh */}
            {onRefresh && (
                <button
                    onClick={onRefresh}
                    disabled={isLoading}
                    className="flex items-center gap-2 bg-slate-700 hover:bg-slate-600 disabled:opacity-40
                     border border-white/10 text-slate-300 text-sm px-3 py-2 rounded-lg transition-colors"
                >
                    <svg className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`} fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                    </svg>
                    Refresh
                </button>
            )}
        </div>
    );
}
