'use client';

import useSWR from 'swr';
import { fetchRiskEvents, fetchStratLogs, fetchStratHealth } from '@/lib/api';
import { RiskEvent, StratLog, StratHealth } from '@/lib/types';
import { fmtDate } from '@/lib/format';
import { DataTable, Column } from '@/components/DataTable';

export default function VerificationPage() {
    const { data: health = [], isLoading: loadingHealth } = useSWR('strat_health', fetchStratHealth, { refreshInterval: 5000 });
    const { data: riskEvents = [], isLoading: loadingRisk } = useSWR('risk_events', fetchRiskEvents, { refreshInterval: 5000 });
    const { data: logs = [], isLoading: loadingLogs } = useSWR('strat_logs', fetchStratLogs, { refreshInterval: 5000 });

    const riskColumns: Column<RiskEvent>[] = [
        {
            key: 'event_time', header: 'Time',
            render: e => <span className="mono text-slate-400 text-xs">{fmtDate(e.event_time)}</span>,
        },
        {
            key: 'check_type', header: 'Check',
            render: e => <span className="font-semibold text-slate-200">{e.check_type}</span>,
        },
        {
            key: 'pass_flag', header: 'Status',
            render: e => (
                <span className={`px-2 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider ${e.pass_flag ? 'bg-emerald-900/40 text-emerald-400 border border-emerald-500/20' : 'bg-rose-900/40 text-rose-400 border border-rose-500/20'}`}>
                    {e.pass_flag ? 'PASS' : 'FAIL'}
                </span>
            ),
        },
        {
            key: 'action_taken', header: 'Action',
            render: e => <span className="text-slate-400 text-xs">{e.action_taken ?? '—'}</span>,
        },
        {
            key: 'current_value', header: 'Value',
            render: e => <span className="mono text-slate-300 text-xs">{e.current_value ?? '—'}</span>,
        },
        {
            key: 'limit_value', header: 'Limit',
            render: e => <span className="mono text-slate-500 text-xs">{e.limit_value ?? '—'}</span>,
        },
        {
            key: 'related_order_id', header: 'Order ID',
            render: e => <span className="mono text-slate-600 text-[10px]">{e.related_order_id ?? '—'}</span>,
        },
    ];

    const logColumns: Column<StratLog>[] = [
        {
            key: 'event_time', header: 'Time',
            render: l => <span className="mono text-slate-400 text-xs">{fmtDate(l.event_time)}</span>,
        },
        {
            key: 'symbol', header: 'Symbol',
            render: l => <span className="font-semibold text-indigo-400">{l.symbol}</span>,
        },
        {
            key: 'event_code', header: 'Event',
            render: l => <span className="text-slate-200 font-medium">{l.event_code}</span>,
        },
        {
            key: 'message', header: 'Message',
            render: l => <span className="text-slate-400 text-sm italic">"{l.message}"</span>,
        },
    ];

    return (
        <div className="p-8 space-y-10 max-w-7xl mx-auto">
            {/* Header */}
            <div className="flex items-end justify-between border-b border-white/5 pb-6">
                <div>
                    <h1 className="text-3xl font-extrabold text-white tracking-tight">System Verification</h1>
                    <p className="text-slate-400 mt-2 flex items-center gap-2">
                        <span className="w-2 h-2 rounded-full bg-indigo-500" />
                        Handbook Alignment & Audit Trail
                    </p>
                </div>
                <div className="text-right">
                    <p className="text-[10px] text-slate-500 uppercase tracking-widest font-bold">Auto-refresh</p>
                    <p className="text-xs text-indigo-400 font-medium">Every 5 seconds</p>
                </div>
            </div>

            {/* Strategy Health Section */}
            <section className="space-y-4">
                <div className="flex items-center gap-2 mb-4">
                    <span className="text-xl">💓</span>
                    <h2 className="text-lg font-bold text-slate-100 uppercase tracking-wide">Strategy heartbeats</h2>
                </div>
                
                {loadingHealth ? (
                    <div className="h-24 animate-pulse bg-white/5 rounded-xl border border-white/5" />
                ) : health.length === 0 ? (
                    <div className="p-8 text-center bg-white/5 rounded-xl border border-dashed border-white/10 text-slate-500">
                        No active strategy health heartbeats found.
                    </div>
                ) : (
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                        {health.map(h => (
                            <div key={h.id} className="bg-[#1a2335] rounded-xl border border-white/5 p-5 shadow-xl hover:border-indigo-500/30 transition-all group">
                                <div className="flex justify-between items-start mb-4">
                                    <div>
                                        <h3 className="font-bold text-slate-100 group-hover:text-indigo-300 transition-colors uppercase tracking-wider">{h.strategy_name}</h3>
                                        <p className="text-[10px] text-slate-500 font-mono mt-0.5">{h.instance_id}</p>
                                    </div>
                                    <span className="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.5)] animate-pulse" />
                                </div>
                                <div className="grid grid-cols-2 gap-4">
                                    <div className="bg-black/20 rounded-lg p-2 border border-white/5">
                                        <p className="text-[10px] text-slate-500 uppercase font-bold">CPU Usage</p>
                                        <p className="text-lg font-bold text-slate-200">{h.cpu_pct ?? '0.00'}%</p>
                                    </div>
                                    <div className="bg-black/20 rounded-lg p-2 border border-white/5">
                                        <p className="text-[10px] text-slate-500 uppercase font-bold">Memory</p>
                                        <p className="text-lg font-bold text-slate-200">{h.mem_mb ?? '0.00'} MB</p>
                                    </div>
                                </div>
                                <div className="mt-4 pt-4 border-t border-white/5 flex justify-between items-center text-[10px]">
                                    <span className="text-slate-500 uppercase font-bold">Last Reported</span>
                                    <span className="text-slate-400 font-mono">{fmtDate(h.reported_at)}</span>
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </section>

            {/* Risk Audit Trail */}
            <section className="space-y-4">
                <div className="flex items-center gap-2 mb-4">
                    <span className="text-xl">🛡️</span>
                    <h2 className="text-lg font-bold text-slate-100 uppercase tracking-wide">Risk Audit Trail</h2>
                </div>
                <div className="bg-[#1a2335] rounded-xl border border-white/5 overflow-hidden shadow-2xl">
                    <DataTable
                        columns={riskColumns}
                        data={riskEvents}
                        keyField="id"
                        isLoading={loadingRisk}
                        emptyMessage="No risk events recorded yet."
                    />
                </div>
            </section>

            {/* Decision Logs */}
            <section className="space-y-4">
                <div className="flex items-center gap-2 mb-4">
                    <span className="text-xl">📝</span>
                    <h2 className="text-lg font-bold text-slate-100 uppercase tracking-wide">Strategy Decision Logs</h2>
                </div>
                <div className="bg-[#1a2335] rounded-xl border border-white/5 overflow-hidden shadow-2xl">
                    <DataTable
                        columns={logColumns}
                        data={logs}
                        keyField="id"
                        isLoading={loadingLogs}
                        emptyMessage="No strategy logs recorded yet."
                    />
                </div>
            </section>
        </div>
    );
}
