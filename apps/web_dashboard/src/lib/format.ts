import { OrderStatus } from './types';

export function fmtDate(iso: string): string {
    const d = new Date(iso);
    return d.toLocaleString('en-GB', {
        day: '2-digit', month: 'short', year: 'numeric',
        hour: '2-digit', minute: '2-digit', second: '2-digit',
        hour12: false,
    });
}

export function fmtDateShort(iso: string): string {
    return new Date(iso).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

export function fmtDecimal(val: string | null | undefined, decimals = 4): string {
    if (!val) return '—';
    return parseFloat(val).toLocaleString('en-US', {
        minimumFractionDigits: 2,
        maximumFractionDigits: decimals,
    });
}

export function statusColor(status: OrderStatus): { bg: string; text: string; dot: string } {
    switch (status) {
        case 'FILLED': return { bg: 'bg-emerald-900/40', text: 'text-emerald-400', dot: 'bg-emerald-400' };
        case 'PARTIALLY_FILLED': return { bg: 'bg-sky-900/40', text: 'text-sky-400', dot: 'bg-sky-400' };
        case 'NEW': return { bg: 'bg-violet-900/40', text: 'text-violet-400', dot: 'bg-violet-400' };
        case 'CANCELED': return { bg: 'bg-slate-700/50', text: 'text-slate-400', dot: 'bg-slate-400' };
        case 'REJECTED': return { bg: 'bg-rose-900/40', text: 'text-rose-400', dot: 'bg-rose-400' };
        case 'EXPIRED': return { bg: 'bg-amber-900/40', text: 'text-amber-400', dot: 'bg-amber-400' };
        default: return { bg: 'bg-slate-700/50', text: 'text-slate-300', dot: 'bg-slate-300' };
    }
}

export function eventIcon(eventType: string): string {
    switch (eventType) {
        case 'SUBMITTED': return '📤';
        case 'ACKNOWLEDGED': return '✅';
        case 'PARTIALLY_FILLED': return '⚡';
        case 'FILLED': return '🎯';
        case 'CANCELED': return '🚫';
        case 'REJECTED': return '❌';
        case 'EXPIRED': return '⏰';
        case 'REPLACE_REQUESTED': return '🔄';
        default: return '📋';
    }
}

export function eventColor(eventType: string): string {
    switch (eventType) {
        case 'SUBMITTED': return 'border-violet-500 text-violet-400';
        case 'ACKNOWLEDGED': return 'border-sky-500 text-sky-400';
        case 'PARTIALLY_FILLED': return 'border-blue-400 text-blue-300';
        case 'FILLED': return 'border-emerald-500 text-emerald-400';
        case 'CANCELED': return 'border-slate-500 text-slate-400';
        case 'REJECTED': return 'border-rose-500 text-rose-400';
        case 'EXPIRED': return 'border-amber-500 text-amber-400';
        default: return 'border-slate-600 text-slate-400';
    }
}
