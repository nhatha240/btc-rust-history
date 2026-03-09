'use client';

import { useState, useMemo } from 'react';
import useSWR from 'swr';
import { fetchSystemLogs, fetchStrategyLogs, fetchRiskLogs, fetchAuditLogs } from '@/lib/api';
import { ErrorLog, StratLog, RiskEventRecord, StrategyConfigAudit } from '@/lib/types';
import { fmtDate } from '@/lib/format';
import { DataTable, Column } from '@/components/DataTable';

type LogTab = 'system' | 'strategy' | 'risk' | 'audit';

export default function LogsPage() {
    const [activeTab, setActiveTab] = useState<LogTab>('system');
    const [serviceFilter, setServiceFilter] = useState('');
    const [severityFilter, setSeverityFilter] = useState('');

    const { data: systemLogs = [], isLoading: loadingSystem } = useSWR(
        activeTab === 'system' ? ['logs/system', serviceFilter, severityFilter] : null,
        () => fetchSystemLogs({ service: serviceFilter || undefined, severity: severityFilter || undefined })
    );

    const { data: strategyLogs = [], isLoading: loadingStrategy } = useSWR(
        activeTab === 'strategy' ? 'logs/strategy' : null,
        () => fetchStrategyLogs()
    );

    const { data: riskLogs = [], isLoading: loadingRisk } = useSWR(
        activeTab === 'risk' ? 'logs/risk' : null,
        () => fetchRiskLogs()
    );

    const { data: auditLogs = [], isLoading: loadingAudit } = useSWR(
        activeTab === 'audit' ? 'logs/audit' : null,
        () => fetchAuditLogs()
    );

    const systemColumns: Column<ErrorLog>[] = [
        { key: 'occurred_at', header: 'Time', render: l => <span className="mono text-slate-500">{fmtDate(l.occurred_at)}</span> },
        {
            key: 'severity', header: 'Severity',
            render: l => {
                const colors = {
                    DEBUG: 'text-slate-500',
                    INFO: 'text-sky-400',
                    WARNING: 'text-amber-400',
                    ERROR: 'text-rose-400',
                    CRITICAL: 'text-rose-600 font-bold bg-rose-900/20 px-1 rounded'
                };
                return <span className={`text-[10px] font-black tracking-widest ${colors[l.severity]}`}>{l.severity}</span>;
            }
        },
        { key: 'service_name', header: 'Service', render: l => <span className="text-indigo-400 font-semibold">{l.service_name}</span> },
        { key: 'message', header: 'Message', render: l => <span className="text-slate-300">{l.message}</span> },
    ];

    const strategyColumns: Column<StratLog>[] = [
        { key: 'event_time', header: 'Time', render: l => <span className="mono text-slate-500">{fmtDate(l.event_time)}</span> },
        { key: 'strategy_version_id', header: 'Strategy', render: l => <span className="text-indigo-300">{l.strategy_version_id}</span> },
        { key: 'symbol', header: 'Symbol', render: l => <span className="font-bold text-white">{l.symbol}</span> },
        { key: 'event_code', header: 'Event', render: l => <span className="text-[10px] bg-slate-800 px-1.5 py-0.5 rounded text-slate-400 mono">{l.event_code}</span> },
        { key: 'message', header: 'Rationale' },
    ];

    const riskColumns: Column<RiskEventRecord>[] = [
        { key: 'created_at', header: 'Time', render: l => <span className="mono text-slate-500">{fmtDate(l.created_at)}</span> },
        {
            key: 'decision', header: 'Decision',
            render: l => (
                <span className={`text-[10px] font-bold px-2 py-0.5 rounded ${l.decision === 'REJECTED' ? 'bg-rose-900/40 text-rose-400' : 'bg-emerald-900/40 text-emerald-400'}`}>
                    {l.decision}
                </span>
            )
        },
        { key: 'account_id', header: 'Account', render: l => <span className="text-slate-400">{l.account_id}</span> },
        { key: 'event_type', header: 'Check', render: l => <span className="text-sky-400 font-medium">{l.event_type}</span> },
        { key: 'reason', header: 'Reason' },
    ];

    const auditColumns: Column<StrategyConfigAudit>[] = [
        { key: 'created_at', header: 'Time', render: l => <span className="mono text-slate-500">{fmtDate(l.created_at)}</span> },
        { key: 'changed_by', header: 'User', render: l => <span className="text-indigo-400 font-bold">{l.changed_by}</span> },
        { key: 'strategy_id', header: 'Strategy', render: l => <span className="text-slate-400 text-xs">{l.strategy_id}</span> },
        { key: 'change_reason', header: 'Reason', render: l => <span className="italic text-slate-400">"{l.change_reason || 'No reason provided'}"</span> },
    ];

    return (
        <div className="p-8 space-y-6">
            <div className="flex justify-between items-end">
                <div>
                    <h1 className="text-3xl font-extrabold text-white tracking-tight">System Logs & Audit</h1>
                    <p className="text-slate-400 mt-1">Centralized observability for all trading operations</p>
                </div>
            </div>

            {/* Tabs */}
            <div className="flex gap-1 bg-slate-900/50 p-1 rounded-xl w-fit border border-white/5">
                {[
                    { id: 'system', label: 'System Logs', icon: '🖥️' },
                    { id: 'strategy', label: 'Strategy rationale', icon: '🧠' },
                    { id: 'risk', label: 'Risk Decisions', icon: '🛡️' },
                    { id: 'audit', label: 'Audit Trail', icon: '📝' },
                ].map(t => (
                    <button
                        key={t.id}
                        onClick={() => setActiveTab(t.id as LogTab)}
                        className={`
                            px-4 py-2 rounded-lg text-sm font-bold flex items-center gap-2 transition-all
                            ${activeTab === t.id ? 'bg-indigo-600 text-white shadow-lg' : 'text-slate-400 hover:text-slate-200 hover:bg-white/5'}
                        `}
                    >
                        <span>{t.icon}</span>
                        {t.label}
                    </button>
                ))}
            </div>

            {/* Main Content */}
            <div className="space-y-4">
                {activeTab === 'system' && (
                    <div className="space-y-4">
                        <div className="flex gap-4">
                            <input
                                type="text" placeholder="Filter service..."
                                className="bg-slate-800 border-white/5 rounded-lg px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-indigo-500"
                                value={serviceFilter} onChange={e => setServiceFilter(e.target.value)}
                            />
                            <select
                                className="bg-slate-800 border-white/5 rounded-lg px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-indigo-500"
                                value={severityFilter} onChange={e => setSeverityFilter(e.target.value)}
                            >
                                <option value="">All Severities</option>
                                <option value="INFO">INFO</option>
                                <option value="WARNING">WARNING</option>
                                <option value="ERROR">ERROR</option>
                                <option value="CRITICAL">CRITICAL</option>
                            </select>
                        </div>
                        <DataTable columns={systemColumns} data={systemLogs} keyField="error_id" isLoading={loadingSystem} />
                    </div>
                )}
                {activeTab === 'strategy' && (
                    <DataTable columns={strategyColumns} data={strategyLogs} keyField="id" isLoading={loadingStrategy} />
                )}
                {activeTab === 'risk' && (
                    <DataTable columns={riskColumns} data={riskLogs} keyField="event_id" isLoading={loadingRisk} />
                )}
                {activeTab === 'audit' && (
                    <DataTable columns={auditColumns} data={auditLogs} keyField="id" isLoading={loadingAudit} />
                )}
            </div>
        </div>
    );
}
