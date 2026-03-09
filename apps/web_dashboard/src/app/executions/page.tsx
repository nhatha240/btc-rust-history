'use client';

import { useMemo, useState } from 'react';
import useSWR from 'swr';
import { fetchTrades } from '@/lib/api';
import { Trade } from '@/lib/types';
import { fmtDate, fmtDecimal } from '@/lib/format';
import { DataTable, Column } from '@/components/DataTable';
import { FiltersBar } from '@/components/FiltersBar';

export default function ExecutionsPage() {
    const [symbol, setSymbol] = useState('');
    const [status, setStatus] = useState(''); // Just to satisfy FiltersBar for now

    const { data: trades = [], isLoading, mutate } = useSWR(
        ['trades', symbol],
        () => fetchTrades({
            symbol: symbol || undefined,
            limit: 500,
        }),
        { refreshInterval: 10_000 }
    );

    const columns: Column<Trade>[] = [
        {
            key: 'trade_time', header: 'Time',
            render: t => <span className="mono text-slate-400">{fmtDate(t.trade_time)}</span>,
        },
        {
            key: 'symbol', header: 'Symbol',
            render: t => <span className="font-semibold text-white">{t.symbol}</span>,
        },
        {
            key: 'side', header: 'Side',
            render: t => (
                <span className={`font-semibold text-xs px-2 py-0.5 rounded ${t.side === 'BUY' ? 'bg-emerald-900/50 text-emerald-300' : 'bg-rose-900/50 text-rose-300'}`}>
                    {t.side}
                </span>
            ),
        },
        {
            key: 'qty', header: 'Quantity', align: 'right' as const,
            render: t => <span className="mono">{fmtDecimal(t.qty, 6)}</span>,
        },
        {
            key: 'price', header: 'Price', align: 'right' as const,
            render: t => (
                <div className="text-right">
                    <span className="mono text-white block">{fmtDecimal(t.price, 2)}</span>
                    {t.order_price && (
                        <span className="text-[10px] text-slate-500 mono block">Order: {fmtDecimal(t.order_price, 2)}</span>
                    )}
                </div>
            )
        },
        {
            key: 'slippage', header: 'Slippage', align: 'right' as const,
            render: t => {
                if (!t.order_price) return <span className="text-slate-600">—</span>;
                const p = parseFloat(t.price);
                const op = parseFloat(t.order_price);
                const slip = ((p / op) - 1) * 10000 * (t.side === 'BUY' ? 1 : -1);
                return (
                    <span className={`mono font-semibold ${Math.abs(slip) > 10 ? 'text-rose-400' : Math.abs(slip) > 2 ? 'text-amber-400' : 'text-emerald-400'}`}>
                        {slip > 0 ? '+' : ''}{slip.toFixed(1)} bps
                    </span>
                );
            }
        },
        {
            key: 'commission', header: 'Fee', align: 'right' as const,
            render: t => <span className="mono text-slate-500">{fmtDecimal(t.commission, 4)} {t.commission_asset}</span>,
        },
        {
            key: 'realized_pnl', header: 'Realized PnL', align: 'right' as const,
            render: t => {
                const isPos = t.realized_pnl && parseFloat(t.realized_pnl) > 0;
                const isNeg = t.realized_pnl && parseFloat(t.realized_pnl) < 0;
                return (
                    <span className={`mono font-semibold ${isPos ? 'text-emerald-400' : isNeg ? 'text-rose-400' : 'text-slate-500'}`}>
                        {fmtDecimal(t.realized_pnl, 2)}
                    </span>
                );
            },
        },
        {
            key: 'client_order_id', header: 'Order ID',
            render: t => <span className="text-slate-500 text-xs mono truncate max-w-[120px] block" title={t.client_order_id}>{t.client_order_id}</span>,
        },
        {
            key: 'is_maker', header: 'Role', align: 'right' as const,
            render: t => (
                <span className={`text-xs px-1.5 py-0.5 rounded ${t.is_maker ? 'bg-indigo-900/40 text-indigo-300' : 'bg-slate-800 text-slate-400'}`}>
                    {t.is_maker ? 'MAKER' : 'TAKER'}
                </span>
            )
        }
    ];

    // Compute basic Slippage Chart Data (Since price comes from executions, not actual order prices in this mock system, we just plot average prices over time)
    const kpi = useMemo(() => {
        const totalTrades = trades.length;
        const totalVolume = trades.reduce((acc, t) => acc + (parseFloat(t.quote_qty) || 0), 0);
        const makerCount = trades.filter(t => t.is_maker).length;
        const totalFees = trades.reduce((acc, t) => acc + (parseFloat(t.commission) || 0), 0);

        return { totalTrades, totalVolume, makerCount, totalFees };
    }, [trades]);

    return (
        <div className="p-6 space-y-6">
            {/* Header */}
            <div>
                <h1 className="text-2xl font-bold text-white">Execution Log & Fills</h1>
                <p className="text-slate-400 text-sm mt-1">Detailed breakdown of all your venue fills</p>
            </div>

            {/* KPI row */}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                {[
                    { label: 'Total Fills', value: kpi.totalTrades, color: 'text-slate-100' },
                    { label: 'Volume (USDT)', value: fmtDecimal(kpi.totalVolume.toString(), 2), color: 'text-indigo-400' },
                    { label: 'Maker Fills', value: kpi.makerCount, color: 'text-sky-400' },
                    { label: 'Paid Fees', value: fmtDecimal(kpi.totalFees.toString(), 2), color: 'text-rose-400' },
                ].map(k => (
                    <div key={k.label} className="card px-5 py-4">
                        <p className="text-xs text-slate-500 uppercase tracking-wider font-semibold">{k.label}</p>
                        <p className={`text-2xl font-bold mt-1 ${k.color}`}>{k.value}</p>
                    </div>
                ))}
            </div>

            {/* Slippage & Volume Chart placeholder - normally we would use a library like Recharts here */}
            <div className="card p-6 min-h-[220px] flex items-center justify-center border-dashed border-slate-700">
                <div className="text-center space-y-2">
                    <span className="text-3xl">📊</span>
                    <h3 className="text-lg font-medium text-slate-300">Execution Price Scatter</h3>
                    <p className="text-sm text-slate-500 max-w-sm mx-auto">
                        In a full implementation, Recharts or lightweight-charts would render slippage tracking over time here.
                    </p>
                </div>
            </div>

            {/* Filters */}
            <div className="flex flex-wrap items-center gap-4 justify-between">
                <FiltersBar
                    symbol={symbol} status={status}
                    onSymbolChange={setSymbol} onStatusChange={setStatus}
                    onRefresh={() => mutate()} isLoading={isLoading}
                />
            </div>

            {/* Table */}
            {isLoading ? (
                <div className="flex items-center justify-center h-48 text-slate-500">
                    <svg className="animate-spin w-6 h-6 mr-2" fill="none" viewBox="0 0 24 24">
                        <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                        <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8z" />
                    </svg>
                    Loading executions…
                </div>
            ) : (
                <DataTable
                    columns={columns}
                    data={trades}
                    keyField="id"
                    emptyMessage="No executions found."
                />
            )}
        </div>
    );
}
