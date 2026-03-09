'use client';

import { useEffect, useMemo, useState } from 'react';
import useSWR, { mutate } from 'swr';
import {
    fetchStrategies,
    fetchStrategyConfigAudit,
    updateStrategyConfig,
} from '@/lib/api';
import { fmtDate } from '@/lib/format';
import { Strategy, StrategyConfigAudit } from '@/lib/types';

type PathToken = string | number;
type OverrideValueType = 'string' | 'number' | 'boolean' | 'null' | 'json';
type DiffKind = 'ADDED' | 'REMOVED' | 'CHANGED';
type OrderRoutingMode = 'AB_TEST_ONLY' | 'REAL_WHEN_CONFIDENCE_GTE';

interface ConfigDiffRow {
    path: string;
    before: unknown;
    after: unknown;
    kind: DiffKind;
}

function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function deepCloneJson(value: unknown): unknown {
    return JSON.parse(JSON.stringify(value ?? {}));
}

function parsePath(path: string): PathToken[] {
    return path
        .trim()
        .replace(/\[(\d+)\]/g, '.$1')
        .split('.')
        .filter(Boolean)
        .map((segment) => (/^\d+$/.test(segment) ? Number(segment) : segment));
}

function serializeValue(value: unknown): string {
    if (typeof value === 'undefined') return '∅';
    if (typeof value === 'string') return value;
    if (
        typeof value === 'number' ||
        typeof value === 'boolean' ||
        value === null
    ) {
        return String(value);
    }

    try {
        return JSON.stringify(value);
    } catch {
        return String(value);
    }
}

function diffConfigs(before: unknown, after: unknown, path = ''): ConfigDiffRow[] {
    if (JSON.stringify(before) === JSON.stringify(after)) {
        return [];
    }

    if (Array.isArray(before) && Array.isArray(after)) {
        const rows: ConfigDiffRow[] = [];
        const maxLen = Math.max(before.length, after.length);

        for (let i = 0; i < maxLen; i += 1) {
            const nextPath = path ? `${path}[${i}]` : `[${i}]`;
            const beforeExists = i < before.length;
            const afterExists = i < after.length;

            if (!beforeExists && afterExists) {
                rows.push({
                    path: nextPath,
                    before: undefined,
                    after: after[i],
                    kind: 'ADDED',
                });
                continue;
            }

            if (beforeExists && !afterExists) {
                rows.push({
                    path: nextPath,
                    before: before[i],
                    after: undefined,
                    kind: 'REMOVED',
                });
                continue;
            }

            rows.push(...diffConfigs(before[i], after[i], nextPath));
        }

        return rows;
    }

    if (isRecord(before) && isRecord(after)) {
        const rows: ConfigDiffRow[] = [];
        const keys = Array.from(
            new Set([...Object.keys(before), ...Object.keys(after)]),
        ).sort();

        for (const key of keys) {
            const nextPath = path ? `${path}.${key}` : key;
            const hasBefore = Object.prototype.hasOwnProperty.call(before, key);
            const hasAfter = Object.prototype.hasOwnProperty.call(after, key);

            if (!hasBefore && hasAfter) {
                rows.push({
                    path: nextPath,
                    before: undefined,
                    after: after[key],
                    kind: 'ADDED',
                });
                continue;
            }

            if (hasBefore && !hasAfter) {
                rows.push({
                    path: nextPath,
                    before: before[key],
                    after: undefined,
                    kind: 'REMOVED',
                });
                continue;
            }

            rows.push(...diffConfigs(before[key], after[key], nextPath));
        }

        return rows;
    }

    return [
        {
            path: path || '(root)',
            before,
            after,
            kind: 'CHANGED',
        },
    ];
}

function setValueAtPath(
    source: unknown,
    tokens: PathToken[],
    value: unknown,
): unknown {
    if (tokens.length === 0) return value;

    const [head, ...rest] = tokens;
    if (typeof head === 'number') {
        const arrayClone = Array.isArray(source) ? [...source] : [];
        arrayClone[head] = setValueAtPath(arrayClone[head], rest, value);
        return arrayClone;
    }

    const objectClone: Record<string, unknown> = isRecord(source)
        ? { ...source }
        : {};
    objectClone[head] = setValueAtPath(objectClone[head], rest, value);
    return objectClone;
}

function removePath(source: unknown, tokens: PathToken[]): unknown {
    if (tokens.length === 0) return source;

    const [head, ...rest] = tokens;
    if (typeof head === 'number') {
        if (!Array.isArray(source)) return source;
        const arrayClone = [...source];

        if (rest.length === 0) {
            if (head >= 0 && head < arrayClone.length) {
                arrayClone.splice(head, 1);
            }
            return arrayClone;
        }

        arrayClone[head] = removePath(arrayClone[head], rest);
        return arrayClone;
    }

    if (!isRecord(source)) return source;
    const objectClone: Record<string, unknown> = { ...source };

    if (rest.length === 0) {
        delete objectClone[head];
        return objectClone;
    }

    objectClone[head] = removePath(objectClone[head], rest);
    return objectClone;
}

function parseOverrideValue(
    rawValue: string,
    valueType: OverrideValueType,
): unknown {
    switch (valueType) {
        case 'string':
            return rawValue;
        case 'number': {
            const parsed = Number(rawValue);
            if (!Number.isFinite(parsed)) {
                throw new Error('Number override must be a finite value.');
            }
            return parsed;
        }
        case 'boolean': {
            const lowered = rawValue.trim().toLowerCase();
            if (lowered !== 'true' && lowered !== 'false') {
                throw new Error('Boolean override only accepts true or false.');
            }
            return lowered === 'true';
        }
        case 'null':
            return null;
        case 'json':
            return JSON.parse(rawValue);
        default:
            return rawValue;
    }
}

export default function SettingsPage() {
    const { data: strategies = [], isLoading: isStrategiesLoading } = useSWR<Strategy[]>(
        'strategies',
        fetchStrategies,
    );

    const [selectedStrategyId, setSelectedStrategyId] = useState('');
    const [changedBy, setChangedBy] = useState('dashboard-operator');
    const [reason, setReason] = useState('');
    const [overridePath, setOverridePath] = useState('');
    const [overrideValue, setOverrideValue] = useState('');
    const [overrideType, setOverrideType] = useState<OverrideValueType>('number');
    const [draftConfig, setDraftConfig] = useState<unknown>({});
    const [editorText, setEditorText] = useState('{}');
    const [selectedAuditId, setSelectedAuditId] = useState<number | null>(null);
    const [orderRoutingMode, setOrderRoutingMode] = useState<OrderRoutingMode>('AB_TEST_ONLY');
    const [realOrderConfidence, setRealOrderConfidence] = useState('0.75');
    const [isSaving, setIsSaving] = useState(false);
    const [rollingBackAuditId, setRollingBackAuditId] = useState<number | null>(null);
    const [error, setError] = useState('');
    const [info, setInfo] = useState('');

    useEffect(() => {
        if (strategies.length === 0) {
            setSelectedStrategyId('');
            return;
        }

        const hasSelected = strategies.some((strategy) => strategy.id === selectedStrategyId);
        if (!hasSelected) {
            setSelectedStrategyId(strategies[0].id);
        }
    }, [strategies, selectedStrategyId]);

    const selectedStrategy = useMemo(
        () => strategies.find((strategy) => strategy.id === selectedStrategyId) ?? null,
        [strategies, selectedStrategyId],
    );

    useEffect(() => {
        if (!selectedStrategy) {
            setDraftConfig({});
            setEditorText('{}');
            return;
        }

        const nextDraft = deepCloneJson(selectedStrategy.config_json);
        setDraftConfig(nextDraft);
        setEditorText(JSON.stringify(nextDraft, null, 2));

        const executionControl = isRecord(nextDraft)
            ? nextDraft.execution_control
            : undefined;
        if (isRecord(executionControl)) {
            const routingMode = executionControl.routing_mode;
            if (
                routingMode === 'AB_TEST_ONLY' ||
                routingMode === 'REAL_WHEN_CONFIDENCE_GTE'
            ) {
                setOrderRoutingMode(routingMode);
            } else {
                setOrderRoutingMode('AB_TEST_ONLY');
            }

            const threshold = executionControl.real_order_confidence_gte;
            if (typeof threshold === 'number' && Number.isFinite(threshold)) {
                setRealOrderConfidence(String(threshold));
            } else {
                setRealOrderConfidence('0.75');
            }
        } else {
            setOrderRoutingMode('AB_TEST_ONLY');
            setRealOrderConfidence('0.75');
        }

        setReason('');
        setInfo('');
        setError('');
    }, [selectedStrategy]);

    const auditKey = selectedStrategy ? `strategy_audit_${selectedStrategy.id}` : null;
    const {
        data: auditLogs = [],
        isLoading: isAuditLoading,
    } = useSWR<StrategyConfigAudit[]>(
        auditKey,
        () => (selectedStrategy ? fetchStrategyConfigAudit(selectedStrategy.id) : Promise.resolve([])),
    );

    useEffect(() => {
        if (auditLogs.length === 0) {
            setSelectedAuditId(null);
            return;
        }

        if (!selectedAuditId || !auditLogs.some((log) => log.id === selectedAuditId)) {
            setSelectedAuditId(auditLogs[0].id);
        }
    }, [auditLogs, selectedAuditId]);

    const pendingDiffs = useMemo(
        () =>
            selectedStrategy
                ? diffConfigs(selectedStrategy.config_json, draftConfig)
                : [],
        [selectedStrategy, draftConfig],
    );

    const selectedAudit = useMemo(
        () => auditLogs.find((log) => log.id === selectedAuditId) ?? null,
        [auditLogs, selectedAuditId],
    );

    const selectedAuditDiffs = useMemo(
        () =>
            selectedAudit
                ? diffConfigs(selectedAudit.old_config, selectedAudit.new_config)
                : [],
        [selectedAudit],
    );

    const onApplyOverride = () => {
        setError('');
        setInfo('');

        const pathTokens = parsePath(overridePath);
        if (pathTokens.length === 0) {
            setError('Override path is required. Example: risk.max_notional_usd');
            return;
        }

        try {
            const parsedValue = parseOverrideValue(overrideValue, overrideType);
            const nextDraft = setValueAtPath(draftConfig, pathTokens, parsedValue);
            setDraftConfig(nextDraft);
            setEditorText(JSON.stringify(nextDraft, null, 2));
            setInfo(`Applied override at path "${overridePath}".`);
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Invalid override value.');
        }
    };

    const onRemovePath = () => {
        setError('');
        setInfo('');

        const pathTokens = parsePath(overridePath);
        if (pathTokens.length === 0) {
            setError('Path is required to remove a parameter.');
            return;
        }

        const nextDraft = removePath(draftConfig, pathTokens);
        setDraftConfig(nextDraft);
        setEditorText(JSON.stringify(nextDraft, null, 2));
        setInfo(`Removed path "${overridePath}" from draft config.`);
    };

    const onApplyEditorJson = () => {
        setError('');
        setInfo('');
        try {
            const parsed = JSON.parse(editorText);
            setDraftConfig(parsed);
            setInfo('Updated draft config from JSON editor.');
        } catch {
            setError('Editor JSON is invalid.');
        }
    };

    const onApplyRoutingPolicy = () => {
        setError('');
        setInfo('');

        const threshold = Number(realOrderConfidence);
        if (!Number.isFinite(threshold) || threshold < 0 || threshold > 1) {
            setError('Confidence threshold phải nằm trong khoảng 0.0 -> 1.0');
            return;
        }

        const executionControl = {
            routing_mode: orderRoutingMode,
            real_order_confidence_gte: Number(threshold.toFixed(4)),
            fallback_mode: 'AB_TEST_ONLY',
            updated_at: new Date().toISOString(),
        };

        const nextDraft = setValueAtPath(draftConfig, ['execution_control'], executionControl);
        setDraftConfig(nextDraft);
        setEditorText(JSON.stringify(nextDraft, null, 2));
        setInfo('Đã áp dụng policy REAL/AB test vào draft config.');
    };

    const onResetDraft = () => {
        if (!selectedStrategy) return;
        const baseline = deepCloneJson(selectedStrategy.config_json);
        setDraftConfig(baseline);
        setEditorText(JSON.stringify(baseline, null, 2));
        setReason('');
        setInfo('Draft was reset to current active config.');
        setError('');
    };

    const onPublishOverrides = async () => {
        if (!selectedStrategy) return;
        if (!reason.trim()) {
            setError('Change reason is required for audit.');
            return;
        }

        setError('');
        setInfo('');
        setIsSaving(true);

        try {
            await updateStrategyConfig(selectedStrategy.id, {
                config: draftConfig,
                changed_by: changedBy.trim() || 'dashboard-operator',
                reason: reason.trim(),
            });

            await Promise.all([
                mutate('strategies'),
                mutate(`strategy_audit_${selectedStrategy.id}`),
            ]);

            setReason('');
            setInfo('Config overrides published successfully.');
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to publish overrides.');
        } finally {
            setIsSaving(false);
        }
    };

    const onRollback = async (audit: StrategyConfigAudit) => {
        if (!selectedStrategy) return;

        const confirmed = window.confirm(
            `Rollback strategy "${selectedStrategy.strategy_name}" using audit #${audit.id}?`,
        );
        if (!confirmed) return;

        setError('');
        setInfo('');
        setRollingBackAuditId(audit.id);

        try {
            const rollbackReason = `Rollback via dashboard from audit #${audit.id}${
                audit.change_reason ? ` (${audit.change_reason})` : ''
            }`;

            await updateStrategyConfig(selectedStrategy.id, {
                config: audit.old_config,
                changed_by: changedBy.trim() || 'dashboard-operator',
                reason: rollbackReason,
            });

            await Promise.all([
                mutate('strategies'),
                mutate(`strategy_audit_${selectedStrategy.id}`),
            ]);

            setInfo(`Rollback from audit #${audit.id} completed.`);
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Rollback failed.');
        } finally {
            setRollingBackAuditId(null);
        }
    };

    return (
        <div className="p-8 max-w-7xl mx-auto space-y-8">
            <div className="border-b border-white/5 pb-6">
                <h1 className="text-3xl font-extrabold text-white tracking-tight">
                    Config & Parameter Management
                </h1>
                <p className="text-slate-400 mt-2">
                    Dynamic overrides, config diff history, and one-click rollback.
                </p>
            </div>

            {error && (
                <div className="rounded-xl border border-rose-500/30 bg-rose-500/10 px-4 py-3 text-sm text-rose-300">
                    {error}
                </div>
            )}
            {info && !error && (
                <div className="rounded-xl border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-300">
                    {info}
                </div>
            )}

            <section className="bg-[#1a2335] rounded-2xl border border-white/5 p-6 space-y-6 shadow-2xl">
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                    <div className="space-y-1">
                        <label className="text-[10px] text-slate-500 uppercase tracking-wider font-bold">
                            Strategy
                        </label>
                        <select
                            className="w-full rounded-lg border border-white/10 bg-slate-900/60 px-3 py-2.5 text-sm text-slate-100"
                            value={selectedStrategyId}
                            onChange={(event) => setSelectedStrategyId(event.target.value)}
                            disabled={isStrategiesLoading || strategies.length === 0}
                        >
                            {strategies.length === 0 && <option value="">No strategy found</option>}
                            {strategies.map((strategy) => (
                                <option key={strategy.id} value={strategy.id}>
                                    {strategy.strategy_name} ({strategy.version})
                                </option>
                            ))}
                        </select>
                    </div>

                    <div className="space-y-1">
                        <label className="text-[10px] text-slate-500 uppercase tracking-wider font-bold">
                            Changed By
                        </label>
                        <input
                            value={changedBy}
                            onChange={(event) => setChangedBy(event.target.value)}
                            placeholder="dashboard-operator"
                            className="w-full rounded-lg border border-white/10 bg-slate-900/60 px-3 py-2.5 text-sm text-slate-100"
                        />
                    </div>

                    <div className="space-y-1">
                        <label className="text-[10px] text-slate-500 uppercase tracking-wider font-bold">
                            Reason (required)
                        </label>
                        <input
                            value={reason}
                            onChange={(event) => setReason(event.target.value)}
                            placeholder="Explain why this override is needed"
                            className="w-full rounded-lg border border-white/10 bg-slate-900/60 px-3 py-2.5 text-sm text-slate-100"
                        />
                    </div>
                </div>

                <div className="rounded-xl border border-sky-500/20 bg-sky-500/5 p-4 space-y-3">
                    <p className="text-xs uppercase tracking-wider font-bold text-sky-300">
                        Order Routing Policy (Real vs AB Test)
                    </p>
                    <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                        <select
                            value={orderRoutingMode}
                            onChange={(event) => setOrderRoutingMode(event.target.value as OrderRoutingMode)}
                            className="rounded-lg border border-white/10 bg-slate-900/60 px-3 py-2.5 text-sm text-slate-100"
                        >
                            <option value="AB_TEST_ONLY">AB_TEST_ONLY (không đặt lệnh thật)</option>
                            <option value="REAL_WHEN_CONFIDENCE_GTE">REAL_WHEN_CONFIDENCE_GTE</option>
                        </select>
                        <input
                            type="number"
                            min="0"
                            max="1"
                            step="0.01"
                            value={realOrderConfidence}
                            onChange={(event) => setRealOrderConfidence(event.target.value)}
                            className="rounded-lg border border-white/10 bg-slate-900/60 px-3 py-2.5 text-sm text-slate-100"
                            placeholder="0.75"
                        />
                        <button
                            onClick={onApplyRoutingPolicy}
                            className="px-4 py-2 rounded-lg border border-sky-500/30 bg-sky-500/10 hover:bg-sky-500/20 text-sky-200 text-sm font-semibold"
                        >
                            Apply Routing Policy
                        </button>
                    </div>
                    <p className="text-xs text-slate-400">
                        Policy sẽ được ghi vào <span className="font-mono text-slate-300">config_json.execution_control</span>.
                        Nếu chọn <span className="font-mono text-slate-300">AB_TEST_ONLY</span> thì mọi tín hiệu nên chỉ chạy shadow/AB test.
                    </p>
                </div>

                <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
                    <input
                        value={overridePath}
                        onChange={(event) => setOverridePath(event.target.value)}
                        placeholder="Path, e.g. risk.max_daily_loss_r or filters[0].threshold"
                        className="rounded-lg border border-white/10 bg-slate-900/60 px-3 py-2.5 text-sm text-slate-100"
                    />
                    <select
                        value={overrideType}
                        onChange={(event) =>
                            setOverrideType(event.target.value as OverrideValueType)
                        }
                        className="rounded-lg border border-white/10 bg-slate-900/60 px-3 py-2.5 text-sm text-slate-100"
                    >
                        <option value="number">number</option>
                        <option value="string">string</option>
                        <option value="boolean">boolean</option>
                        <option value="null">null</option>
                        <option value="json">json</option>
                    </select>
                    <input
                        value={overrideValue}
                        onChange={(event) => setOverrideValue(event.target.value)}
                        placeholder={overrideType === 'null' ? 'Value ignored for null' : 'Override value'}
                        disabled={overrideType === 'null'}
                        className="rounded-lg border border-white/10 bg-slate-900/60 px-3 py-2.5 text-sm text-slate-100 disabled:opacity-50"
                    />
                </div>

                <div className="flex flex-wrap gap-2">
                    <button
                        onClick={onApplyOverride}
                        className="px-4 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white text-sm font-semibold"
                    >
                        Apply Override
                    </button>
                    <button
                        onClick={onRemovePath}
                        className="px-4 py-2 rounded-lg border border-amber-500/30 bg-amber-500/10 hover:bg-amber-500/20 text-amber-300 text-sm font-semibold"
                    >
                        Remove Path
                    </button>
                    <button
                        onClick={onResetDraft}
                        className="px-4 py-2 rounded-lg border border-white/10 bg-white/5 hover:bg-white/10 text-slate-200 text-sm font-semibold"
                    >
                        Reset Draft
                    </button>
                    <button
                        onClick={onPublishOverrides}
                        disabled={isSaving || !selectedStrategy || pendingDiffs.length === 0}
                        className="px-4 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 disabled:cursor-not-allowed text-white text-sm font-semibold"
                    >
                        {isSaving ? 'Publishing...' : 'Publish Overrides'}
                    </button>
                </div>
            </section>

            <section className="grid grid-cols-1 xl:grid-cols-2 gap-6">
                <div className="bg-[#1a2335] rounded-2xl border border-white/5 p-6 space-y-4 shadow-2xl">
                    <div className="flex items-center justify-between">
                        <h2 className="text-lg font-bold text-white">Pending Config Diff</h2>
                        <span className="text-xs text-slate-400">
                            {pendingDiffs.length} field(s) changed
                        </span>
                    </div>

                    <div className="max-h-80 overflow-auto rounded-xl border border-white/10">
                        {pendingDiffs.length === 0 ? (
                            <div className="px-4 py-8 text-center text-slate-500 text-sm">
                                No pending override.
                            </div>
                        ) : (
                            <table className="w-full text-sm">
                                <thead className="bg-slate-900/60">
                                    <tr>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">Path</th>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">Before</th>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">After</th>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">Type</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {pendingDiffs.map((row) => (
                                        <tr key={`${row.path}-${row.kind}`} className="border-t border-white/5">
                                            <td className="px-3 py-2 text-indigo-300 font-mono text-xs">{row.path}</td>
                                            <td className="px-3 py-2 text-slate-400 font-mono text-xs break-all">{serializeValue(row.before)}</td>
                                            <td className="px-3 py-2 text-slate-200 font-mono text-xs break-all">{serializeValue(row.after)}</td>
                                            <td className="px-3 py-2 text-xs">
                                                <span className={`px-2 py-0.5 rounded border ${
                                                    row.kind === 'ADDED'
                                                        ? 'bg-emerald-500/10 text-emerald-300 border-emerald-500/30'
                                                        : row.kind === 'REMOVED'
                                                            ? 'bg-amber-500/10 text-amber-300 border-amber-500/30'
                                                            : 'bg-sky-500/10 text-sky-300 border-sky-500/30'
                                                }`}>
                                                    {row.kind}
                                                </span>
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        )}
                    </div>
                </div>

                <div className="bg-[#1a2335] rounded-2xl border border-white/5 p-6 space-y-3 shadow-2xl">
                    <div className="flex items-center justify-between">
                        <h2 className="text-lg font-bold text-white">Draft JSON Editor</h2>
                        <button
                            onClick={onApplyEditorJson}
                            className="px-3 py-1.5 rounded-lg border border-indigo-500/30 bg-indigo-500/10 hover:bg-indigo-500/20 text-indigo-300 text-xs font-semibold"
                        >
                            Apply JSON
                        </button>
                    </div>
                    <textarea
                        value={editorText}
                        onChange={(event) => setEditorText(event.target.value)}
                        className="w-full h-80 rounded-xl border border-white/10 bg-slate-900/60 p-4 text-xs font-mono text-slate-200"
                    />
                </div>
            </section>

            <section className="bg-[#1a2335] rounded-2xl border border-white/5 p-6 space-y-4 shadow-2xl">
                <div className="flex items-center justify-between">
                    <h2 className="text-lg font-bold text-white">Config History & Rollback</h2>
                    <span className="text-xs text-slate-400">
                        {isAuditLoading ? 'Loading history...' : `${auditLogs.length} audit record(s)`}
                    </span>
                </div>

                <div className="grid grid-cols-1 xl:grid-cols-2 gap-5">
                    <div className="max-h-[22rem] overflow-auto rounded-xl border border-white/10">
                        {auditLogs.length === 0 ? (
                            <div className="px-4 py-8 text-center text-slate-500 text-sm">
                                No audit history for this strategy.
                            </div>
                        ) : (
                            <table className="w-full text-sm">
                                <thead className="bg-slate-900/60">
                                    <tr>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">ID</th>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">Changed By</th>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">Time</th>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">Reason</th>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">Actions</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {auditLogs.map((audit) => (
                                        <tr
                                            key={audit.id}
                                            className={`border-t border-white/5 ${
                                                selectedAuditId === audit.id ? 'bg-indigo-500/10' : ''
                                            }`}
                                        >
                                            <td className="px-3 py-2 text-slate-300 font-mono text-xs">{audit.id}</td>
                                            <td className="px-3 py-2 text-slate-200 text-xs">{audit.changed_by}</td>
                                            <td className="px-3 py-2 text-slate-400 text-xs font-mono">{fmtDate(audit.created_at)}</td>
                                            <td className="px-3 py-2 text-slate-300 text-xs">{audit.change_reason || '—'}</td>
                                            <td className="px-3 py-2">
                                                <div className="flex gap-2">
                                                    <button
                                                        onClick={() => setSelectedAuditId(audit.id)}
                                                        className="px-2 py-1 rounded border border-slate-500/30 bg-slate-500/10 text-slate-200 text-[11px]"
                                                    >
                                                        View Diff
                                                    </button>
                                                    <button
                                                        onClick={() => onRollback(audit)}
                                                        disabled={rollingBackAuditId === audit.id}
                                                        className="px-2 py-1 rounded border border-rose-500/30 bg-rose-500/10 text-rose-300 text-[11px] disabled:opacity-50"
                                                    >
                                                        {rollingBackAuditId === audit.id ? 'Rolling back...' : 'Rollback'}
                                                    </button>
                                                </div>
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        )}
                    </div>

                    <div className="rounded-xl border border-white/10 overflow-auto">
                        {!selectedAudit ? (
                            <div className="px-4 py-8 text-center text-slate-500 text-sm">
                                Select an audit entry to inspect its diff.
                            </div>
                        ) : (
                            <table className="w-full text-sm">
                                <thead className="bg-slate-900/60">
                                    <tr>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">Path</th>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">Before</th>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">After</th>
                                        <th className="px-3 py-2 text-left text-xs text-slate-400">Type</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {selectedAuditDiffs.length === 0 ? (
                                        <tr>
                                            <td colSpan={4} className="px-3 py-8 text-center text-slate-500 text-sm">
                                                No field-level change detected.
                                            </td>
                                        </tr>
                                    ) : (
                                        selectedAuditDiffs.map((row) => (
                                            <tr key={`${selectedAudit.id}-${row.path}-${row.kind}`} className="border-t border-white/5">
                                                <td className="px-3 py-2 text-indigo-300 font-mono text-xs">{row.path}</td>
                                                <td className="px-3 py-2 text-slate-400 font-mono text-xs break-all">{serializeValue(row.before)}</td>
                                                <td className="px-3 py-2 text-slate-200 font-mono text-xs break-all">{serializeValue(row.after)}</td>
                                                <td className="px-3 py-2 text-xs">
                                                    <span className={`px-2 py-0.5 rounded border ${
                                                        row.kind === 'ADDED'
                                                            ? 'bg-emerald-500/10 text-emerald-300 border-emerald-500/30'
                                                            : row.kind === 'REMOVED'
                                                                ? 'bg-amber-500/10 text-amber-300 border-amber-500/30'
                                                                : 'bg-sky-500/10 text-sky-300 border-sky-500/30'
                                                    }`}>
                                                        {row.kind}
                                                    </span>
                                                </td>
                                            </tr>
                                        ))
                                    )}
                                </tbody>
                            </table>
                        )}
                    </div>
                </div>
            </section>
        </div>
    );
}
