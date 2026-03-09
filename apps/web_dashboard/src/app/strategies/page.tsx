'use client';

import { useState } from 'react';
import useSWR, { mutate } from 'swr';
import { fetchStrategies, updateStrategyAction, updateStrategyConfig } from '@/lib/api';
import { Strategy, StrategyStatus } from '@/lib/types';
import { fmtDate } from '@/lib/format';

export default function StrategiesPage() {
    const { data: strategies = [], isLoading } = useSWR<Strategy[]>('strategies', fetchStrategies);
    const [selectedStrategy, setSelectedStrategy] = useState<Strategy | null>(null);
    const [isEditing, setIsEditing] = useState(false);
    const [configText, setConfigText] = useState('');
    const [changeReason, setChangeReason] = useState('');
    const [isSaving, setIsSaving] = useState(false);

    const handleAction = async (id: string, action: string) => {
        const ok = await updateStrategyAction(id, action);
        if (ok) mutate('strategies');
    };

    const startEdit = (strat: Strategy) => {
        setSelectedStrategy(strat);
        setConfigText(JSON.stringify(strat.config_json, null, 2));
        setChangeReason('');
        setIsEditing(true);
    };

    const saveConfig = async () => {
        if (!selectedStrategy) return;
        setIsSaving(true);
        try {
            const newConfig = JSON.parse(configText);
            const ok = await updateStrategyConfig(selectedStrategy.id, {
                config: newConfig,
                changed_by: 'admin', // Placeholder
                reason: changeReason
            });
            if (ok) {
                mutate('strategies');
                setIsEditing(false);
            }
        } catch (e) {
            alert('Invalid JSON');
        } finally {
            setIsSaving(false);
        }
    };

    return (
        <div className="p-8 space-y-8 max-w-7xl mx-auto">
            {/* Header */}
            <div className="flex items-end justify-between border-b border-white/5 pb-6">
                <div>
                    <h1 className="text-3xl font-extrabold text-white tracking-tight">Strategy Management</h1>
                    <p className="text-slate-400 mt-2 flex items-center gap-2">
                        <span className="w-2 h-2 rounded-full bg-indigo-500" />
                        Control, Versioning & Audit Trail
                    </p>
                </div>
            </div>

            {isLoading ? (
                <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                    {[1, 2].map(i => <div key={i} className="h-64 animate-pulse bg-white/5 rounded-2xl border border-white/5" />)}
                </div>
            ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                    {strategies.map(strat => (
                        <div key={strat.id} className="bg-[#1a2335] rounded-2xl border border-white/5 overflow-hidden shadow-2xl flex flex-col">
                            {/* Card Content */}
                            <div className="p-6 flex-1">
                                <div className="flex justify-between items-start mb-4">
                                    <div>
                                        <div className="flex items-center gap-2">
                                            <h2 className="text-xl font-bold text-white uppercase tracking-wide">{strat.strategy_name}</h2>
                                            <span className="text-[10px] bg-indigo-500/20 text-indigo-400 px-1.5 py-0.5 rounded font-mono border border-indigo-500/20">
                                                {strat.version}
                                            </span>
                                        </div>
                                        <p className="text-sm text-slate-500 mt-1">{strat.description ?? 'No description'}</p>
                                    </div>
                                    <StatusBadge status={strat.status} />
                                </div>

                                <div className="grid grid-cols-2 gap-4 mt-6">
                                    <div className="bg-black/20 rounded-xl p-3 border border-white/5">
                                        <p className="text-[10px] text-slate-500 uppercase font-black tracking-widest">Mode</p>
                                        <p className={`text-sm font-bold mt-1 ${strat.mode === 'LIVE' ? 'text-rose-400' : strat.mode === 'PAPER' ? 'text-amber-400' : 'text-slate-400'}`}>
                                            {strat.mode}
                                        </p>
                                    </div>
                                    <div className="bg-black/20 rounded-xl p-3 border border-white/5">
                                        <p className="text-[10px] text-slate-500 uppercase font-black tracking-widest">Last Update</p>
                                        <p className="text-sm font-medium text-slate-300 mt-1 font-mono">
                                            {fmtDate(strat.updated_at).split(' ')[0]}
                                        </p>
                                    </div>
                                </div>

                                <div className="mt-6">
                                    <p className="text-[10px] text-slate-500 uppercase font-black tracking-widest mb-2">Configuration Preview</p>
                                    <pre className="bg-black/40 rounded-xl p-4 text-[11px] font-mono text-indigo-300 border border-white/5 overflow-x-auto max-h-32">
                                        {JSON.stringify(strat.config_json, null, 2)}
                                    </pre>
                                </div>
                            </div>

                            {/* Actions */}
                            <div className="bg-black/20 px-6 py-4 border-t border-white/5 flex gap-2">
                                {strat.status === 'RUNNING' ? (
                                    <>
                                        <button
                                            onClick={() => handleAction(strat.id, 'PAUSE')}
                                            className="px-4 py-2 bg-amber-500/10 text-amber-500 border border-amber-500/20 rounded-lg text-xs font-bold uppercase tracking-wider hover:bg-amber-500/20 transition-all"
                                        >
                                            Pause
                                        </button>
                                        <button
                                            onClick={() => handleAction(strat.id, 'STOP')}
                                            className="px-4 py-2 bg-rose-500/10 text-rose-500 border border-rose-500/20 rounded-lg text-xs font-bold uppercase tracking-wider hover:bg-rose-500/20 transition-all"
                                        >
                                            Stop
                                        </button>
                                    </>
                                ) : (
                                    <button
                                        onClick={() => handleAction(strat.id, 'START')}
                                        className="px-4 py-2 bg-emerald-500/10 text-emerald-500 border border-emerald-500/20 rounded-lg text-xs font-bold uppercase tracking-wider hover:bg-emerald-500/20 transition-all flex-1"
                                    >
                                        Start Strategy
                                    </button>
                                )}
                                <button
                                    onClick={() => startEdit(strat)}
                                    className="px-4 py-2 bg-indigo-500/10 text-indigo-400 border border-indigo-500/20 rounded-lg text-xs font-bold uppercase tracking-wider hover:bg-indigo-500/20 transition-all"
                                >
                                    Edit Config
                                </button>
                            </div>
                        </div>
                    ))}
                </div>
            )}

            {/* Edit Modal */}
            {isEditing && selectedStrategy && (
                <div className="fixed inset-0 bg-slate-950/80 backdrop-blur-sm z-50 flex items-center justify-center p-4">
                    <div className="bg-[#1a2335] w-full max-w-2xl rounded-2xl border border-white/10 shadow-2xl flex flex-col max-h-[90vh]">
                        <div className="p-6 border-b border-white/5 flex justify-between items-center">
                            <div>
                                <h2 className="text-xl font-bold text-white uppercase tracking-tight">Edit Configuration</h2>
                                <p className="text-xs text-slate-500 font-mono mt-0.5">{selectedStrategy.strategy_name} / {selectedStrategy.id}</p>
                            </div>
                            <button onClick={() => setIsEditing(false)} className="text-slate-400 hover:text-white text-2xl">&times;</button>
                        </div>

                        <div className="p-6 flex-1 overflow-y-auto space-y-6">
                            <div>
                                <label className="block text-[10px] text-slate-500 uppercase font-black tracking-widest mb-2 italic">Parameters (JSON)</label>
                                <textarea
                                    value={configText}
                                    onChange={(e) => setConfigText(e.target.value)}
                                    className="w-full h-64 bg-black/40 border border-white/5 rounded-xl p-4 font-mono text-sm text-indigo-300 focus:ring-1 focus:ring-indigo-500 focus:border-indigo-500 outline-none"
                                />
                            </div>

                            <div>
                                <label className="block text-[10px] text-slate-500 uppercase font-black tracking-widest mb-2 italic">Reason for Change (Audit Log)</label>
                                <input
                                    type="text"
                                    placeholder="e.g. Tightening SL due to high vol"
                                    value={changeReason}
                                    onChange={(e) => setChangeReason(e.target.value)}
                                    className="w-full bg-black/40 border border-white/5 rounded-xl px-4 py-3 text-sm text-slate-200 focus:ring-1 focus:ring-indigo-500 focus:border-indigo-500 outline-none"
                                />
                            </div>
                        </div>

                        <div className="p-6 bg-black/20 border-t border-white/5 flex justify-end gap-3">
                            <button
                                onClick={() => setIsEditing(false)}
                                className="px-5 py-2.5 text-slate-400 hover:text-white text-sm font-bold uppercase transition-all"
                            >
                                Cancel
                            </button>
                            <button
                                onClick={saveConfig}
                                disabled={isSaving || !changeReason.trim()}
                                className="px-8 py-2.5 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-xl text-sm font-bold uppercase tracking-wider shadow-lg shadow-indigo-600/20 transition-all"
                            >
                                {isSaving ? 'Saving...' : 'Save & Publish'}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}

function StatusBadge({ status }: { status: StrategyStatus }) {
    const colors = {
        RUNNING: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20',
        PAUSED: 'bg-amber-500/10 text-amber-400 border-amber-500/20',
        HALTED: 'bg-rose-500/10 text-rose-400 border-rose-500/20',
        ERROR: 'bg-rose-600 text-white border-transparent pulse',
    };

    return (
        <span className={`px-2 py-0.5 rounded text-[10px] font-bold uppercase tracking-widest border ${colors[status]}`}>
            {status}
        </span>
    );
}
