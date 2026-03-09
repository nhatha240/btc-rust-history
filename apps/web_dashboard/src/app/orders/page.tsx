'use client';

import { useCallback, useMemo, useState } from 'react';
import { useRouter } from 'next/navigation';
import useSWR from 'swr';
import { fetchOrders, cancelOrder, cancelAllOrders } from '@/lib/api';
import { Order, OrderStatus } from '@/lib/types';
import { fmtDate, fmtDecimal } from '@/lib/format';
import { DataTable, Column } from '@/components/DataTable';
import { OrderStatusBadge } from '@/components/OrderStatusBadge';
import { FiltersBar } from '@/components/FiltersBar';
import { TimeRangePicker } from '@/components/TimeRangePicker';

export default function OrdersPage() {
    const router = useRouter();
    const [symbol, setSymbol] = useState('');
    const [status, setStatus] = useState('');
    const [timeRange, setTimeRange] = useState(0);

    const { data: orders = [], isLoading, mutate } = useSWR(
        ['orders', symbol, status],
        () => fetchOrders({
            symbol: symbol || undefined,
            status: (status as OrderStatus) || undefined,
            limit: 100,
        }),
        { refreshInterval: 30_000 }
    );

    // Client-side time filter
    const filtered = useMemo(() => {
        if (!timeRange) return orders;
        const cutoff = Date.now() - timeRange * 3_600_000;
        return orders.filter(o => new Date(o.created_at).getTime() >= cutoff);
    }, [orders, timeRange]);

    const columns: Column<Order>[] = [
        {
            key: 'created_at', header: 'Time',
            render: o => <span className="mono text-slate-400">{fmtDate(o.created_at)}</span>,
        },
        {
            key: 'symbol', header: 'Symbol',
            render: o => <span className="font-semibold text-white">{o.symbol}</span>,
        },
        {
            key: 'side', header: 'Side',
            render: o => (
                <span className={`font-semibold text-xs px-2 py-0.5 rounded ${o.side === 'BUY' ? 'bg-emerald-900/50 text-emerald-300' : 'bg-rose-900/50 text-rose-300'}`}>
                    {o.side}
                </span>
            ),
        },
        { key: 'type', header: 'Type', render: o => <span className="text-slate-400 text-xs">{o.type}</span> },
        { key: 'tif', header: 'TIF', render: o => <span className="text-slate-500 text-xs">{o.tif}</span> },
        {
            key: 'qty', header: 'Qty', align: 'right' as const,
            render: o => <span className="mono">{fmtDecimal(o.qty, 6)}</span>,
        },
        {
            key: 'price', header: 'Price', align: 'right' as const,
            render: o => <span className="mono">{fmtDecimal(o.price, 2)}</span>,
        },
        {
            key: 'filled_qty', header: 'Filled', align: 'right' as const,
            render: o => (
                <div className="text-right">
                    <span className="mono">{fmtDecimal(o.filled_qty, 6)}</span>
                    {o.qty && (
                        <div className="w-full bg-slate-700 rounded-full h-0.5 mt-1">
                            <div
                                className="bg-indigo-400 h-0.5 rounded-full"
                                style={{ width: `${Math.min(100, (parseFloat(o.filled_qty) / parseFloat(o.qty)) * 100)}%` }}
                            />
                        </div>
                    )}
                </div>
            ),
        },
        {
            key: 'avg_price', header: 'Avg Price', align: 'right' as const,
            render: o => <span className="mono text-slate-400">{fmtDecimal(o.avg_price, 2)}</span>,
        },
        {
            key: 'status', header: 'Status',
            render: o => <OrderStatusBadge status={o.status} />,
        },
        {
            key: 'strategy_version', header: 'Strategy',
            render: o => <span className="text-slate-500 text-xs">{o.strategy_version ?? '—'}</span>,
        },
        {
            key: 'actions', header: '', align: 'right' as const,
            render: o => {
                if (['NEW', 'PARTIALLY_FILLED', 'PartiallyFilled', 'New'].includes(o.status)) {
                    return (
                        <button
                            onClick={(e) => {
                                e.stopPropagation();
                                handleCancel(o.client_order_id);
                            }}
                            className="bg-rose-500/10 hover:bg-rose-500/20 text-rose-400 px-2 py-1 rounded text-xs font-semibold transition-colors"
                        >
                            Cancel
                        </button>
                    );
                }
                return null;
            }
        }
    ];

    const handleCancel = async (id: string) => {
        try {
            await cancelOrder(id);
            mutate();
        } catch (error) {
            console.error('Cancel order failed:', error);
            alert('Failed to cancel order');
        }
    };

    const handleCancelAll = async () => {
        if (!confirm('Are you sure you want to cancel ALL active orders?')) return;
        try {
            await cancelAllOrders({ symbol: symbol || undefined });
            mutate();
        } catch (error) {
            console.error('Cancel all orders failed:', error);
            alert('Failed to cancel all orders');
        }
    };

    const handleRowClick = useCallback((row: Order) => {
        router.push(`/orders/${row.client_order_id}`);
    }, [router]);

    // KPI summary
    const kpi = useMemo(() => ({
        total: filtered.length,
        filled: filtered.filter(o => o.status === 'FILLED').length,
        active: filtered.filter(o => o.status === 'NEW' || o.status === 'PARTIALLY_FILLED').length,
        rejected: filtered.filter(o => o.status === 'REJECTED' || o.status === 'CANCELED').length,
    }), [filtered]);

    return (
        <div className="p-6 space-y-6">
            {/* Header */}
            <div className="flex justify-between items-end">
                <div>
                    <h1 className="text-2xl font-bold text-white">Order History</h1>
                    <p className="text-slate-400 text-sm mt-1">All orders placed by your trading bots</p>
                </div>
                {kpi.active > 0 && (
                    <button
                        onClick={handleCancelAll}
                        className="bg-rose-500 hover:bg-rose-600 text-white font-medium px-4 py-2 rounded shadow-lg shadow-rose-900/20 transition-all flex items-center gap-2"
                    >
                        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                        </svg>
                        Cancel All Active
                    </button>
                )}
            </div>

            {/* KPI row */}
            <div className="grid grid-cols-4 gap-4">
                {[
                    { label: 'Total Orders', value: kpi.total, color: 'text-slate-100' },
                    { label: 'Filled', value: kpi.filled, color: 'text-emerald-400' },
                    { label: 'Active', value: kpi.active, color: 'text-sky-400' },
                    { label: 'Closed/Rejected', value: kpi.rejected, color: 'text-rose-400' },
                ].map(k => (
                    <div key={k.label} className="card px-5 py-4">
                        <p className="text-xs text-slate-500 uppercase tracking-wider font-semibold">{k.label}</p>
                        <p className={`text-3xl font-bold mt-1 ${k.color}`}>{k.value}</p>
                    </div>
                ))}
            </div>

            {/* Filters */}
            <div className="flex flex-wrap items-center gap-4 justify-between">
                <FiltersBar
                    symbol={symbol} status={status}
                    onSymbolChange={setSymbol} onStatusChange={setStatus}
                    onRefresh={() => mutate()} isLoading={isLoading}
                />
                <TimeRangePicker value={timeRange} onChange={setTimeRange} />
            </div>

            {/* Table */}
            {isLoading ? (
                <div className="flex items-center justify-center h-48 text-slate-500">
                    <svg className="animate-spin w-6 h-6 mr-2" fill="none" viewBox="0 0 24 24">
                        <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                        <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8z" />
                    </svg>
                    Loading orders…
                </div>
            ) : (
                <DataTable
                    columns={columns}
                    data={filtered}
                    keyField="client_order_id"
                    onRowClick={handleRowClick}
                    emptyMessage="No orders found for the selected filters"
                />
            )}

            {/* Footer */}
            <p className="text-xs text-slate-600 text-right">
                {filtered.length} order{filtered.length !== 1 ? 's' : ''} · Auto-refresh 30s
            </p>
        </div>
    );
}
