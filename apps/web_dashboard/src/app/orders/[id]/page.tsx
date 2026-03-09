import { notFound } from 'next/navigation';
import Link from 'next/link';
import { fetchOrder, fetchOrderEvents } from '@/lib/api';
import { fmtDate, fmtDecimal, eventIcon, eventColor } from '@/lib/format';
import { OrderStatusBadge } from '@/components/OrderStatusBadge';
import { CancelOrderButton } from '@/components/CancelOrderButton';

interface Props {
    params: { id: string };
}

export default async function OrderDetailPage({ params }: Props) {
    const [order, events] = await Promise.all([
        fetchOrder(params.id),
        fetchOrderEvents(params.id),
    ]);

    if (!order) notFound();

    const fillPct = order.qty
        ? Math.round((parseFloat(order.filled_qty) / parseFloat(order.qty)) * 100)
        : 0;

    return (
        <div className="p-6 space-y-6 max-w-5xl mx-auto">
            {/* Back */}
            <Link href="/orders" className="flex items-center gap-2 text-slate-400 hover:text-slate-200 text-sm transition-colors w-fit">
                <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
                </svg>
                Back to Orders
            </Link>

            {/* Summary header */}
            <div className="card p-5">
                <div className="flex items-start justify-between gap-4 flex-wrap">
                    <div>
                        <div className="flex items-center gap-3">
                            <h1 className="text-xl font-bold text-white">{order.symbol}</h1>
                            <span className={`text-xs font-bold px-2.5 py-1 rounded-md ${order.side === 'BUY' ? 'bg-emerald-900/60 text-emerald-300' : 'bg-rose-900/60 text-rose-300'}`}>
                                {order.side}
                            </span>
                            <OrderStatusBadge status={order.status} />
                            <div className="ml-2">
                                <CancelOrderButton clientOrderId={order.client_order_id} status={order.status} />
                            </div>
                        </div>
                        <p className="mono text-slate-500 text-xs mt-1">{order.client_order_id}</p>
                    </div>
                    <div className="text-right text-sm text-slate-400 space-y-1">
                        <p>Created: <span className="text-slate-300">{fmtDate(order.created_at)}</span></p>
                        <p>Updated: <span className="text-slate-300">{fmtDate(order.updated_at)}</span></p>
                        {order.strategy_version && (
                            <p>Strategy: <span className="text-indigo-400">{order.strategy_version}</span></p>
                        )}
                    </div>
                </div>

                {/* Details grid */}
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 mt-5">
                    {[
                        { label: 'Type', value: order.type },
                        { label: 'TIF', value: order.tif },
                        { label: 'Quantity', value: fmtDecimal(order.qty, 6) },
                        { label: 'Price', value: fmtDecimal(order.price, 2) },
                        { label: 'Avg Fill Price', value: fmtDecimal(order.avg_price, 2) },
                        { label: 'Filled Qty', value: fmtDecimal(order.filled_qty, 6) },
                        { label: 'Reduce Only', value: order.reduce_only ? 'Yes' : 'No' },
                        { label: 'Exchange ID', value: order.exchange_order_id != null ? String(order.exchange_order_id) : '—' },
                        {
                            label: 'Internal Latency',
                            value: order.ack_at ? `${new Date(order.ack_at).getTime() - new Date(order.created_at).getTime()}ms` : '—'
                        },
                        {
                            label: 'Execution Time',
                            value: (order.done_at && order.ack_at) ? `${(new Date(order.done_at).getTime() - new Date(order.ack_at).getTime()) / 1000}s` : '—'
                        },
                    ].map(f => (
                        <div key={f.label} className="bg-slate-800/40 rounded-lg px-4 py-3">
                            <p className="text-xs text-slate-500 uppercase tracking-wider font-semibold">{f.label}</p>
                            <p className="mono text-slate-200 text-sm mt-1">{f.value}</p>
                        </div>
                    ))}
                </div>

                {/* Fill progress */}
                <div className="mt-4">
                    <div className="flex items-center justify-between text-xs text-slate-400 mb-1">
                        <span>Fill Progress</span>
                        <span className="font-semibold text-slate-200">{fillPct}%</span>
                    </div>
                    <div className="h-2 bg-slate-700 rounded-full overflow-hidden">
                        <div
                            className="h-2 rounded-full bg-gradient-to-r from-indigo-500 to-indigo-400 transition-all"
                            style={{ width: `${fillPct}%` }}
                        />
                    </div>
                </div>
            </div>

            {/* Event Timeline */}
            <div>
                <h2 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
                    <span>📜</span> Order Timeline
                    <span className="text-sm font-normal text-slate-500 ml-1">({events.length} events)</span>
                </h2>

                {events.length === 0 ? (
                    <div className="card px-6 py-10 text-center text-slate-500">No events recorded yet</div>
                ) : (
                    <div className="relative">
                        {/* vertical line */}
                        <div className="absolute left-5 top-4 bottom-4 w-px bg-slate-700" />

                        <div className="space-y-3">
                            {events.map((ev, idx) => {
                                const colorCls = eventColor(ev.event_type);
                                const isLast = idx === events.length - 1;
                                return (
                                    <div key={ev.id} className="flex gap-4 relative">
                                        {/* dot */}
                                        <div className={`
                      w-10 h-10 flex-shrink-0 rounded-full flex items-center justify-center text-base z-10
                      bg-slate-900 border-2 ${colorCls.split(' ')[0]}
                      ${isLast ? 'shadow-lg shadow-indigo-900/50' : ''}
                    `}>
                                            {eventIcon(ev.event_type)}
                                        </div>

                                        {/* card */}
                                        <div className="flex-1 card px-4 py-3 mb-0">
                                            <div className="flex items-start justify-between gap-2 flex-wrap">
                                                <div>
                                                    <span className={`text-sm font-bold ${colorCls.split(' ')[1]}`}>
                                                        {ev.event_type.replace(/_/g, ' ')}
                                                    </span>
                                                </div>
                                                <span className="mono text-xs text-slate-500">{fmtDate(ev.event_time)}</span>
                                            </div>

                                            {/* Payload */}
                                            {Object.keys(ev.payload).length > 0 && (
                                                <div className="mt-2 grid grid-cols-2 sm:grid-cols-3 gap-x-4 gap-y-1">
                                                    {Object.entries(ev.payload).map(([k, v]) => (
                                                        <div key={k}>
                                                            <span className="text-[10px] uppercase tracking-wider text-slate-600">{k.replace(/_/g, ' ')}</span>
                                                            <p className="mono text-xs text-slate-300 truncate">{String(v)}</p>
                                                        </div>
                                                    ))}
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
}
