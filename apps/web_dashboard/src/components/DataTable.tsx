'use client';

import { ReactNode } from 'react';

export interface Column<T> {
    key: keyof T | string;
    header: string;
    width?: string;
    align?: 'left' | 'right' | 'center';
    render?: (row: T) => ReactNode;
}

interface Props<T> {
    columns: Column<T>[];
    data: T[];
    keyField: keyof T;
    onRowClick?: (row: T) => void;
    emptyMessage?: string;
    isLoading?: boolean;
}

export function DataTable<T>({ columns, data, keyField, onRowClick, emptyMessage = 'No data', isLoading }: Props<T>) {
    if (isLoading) {
        return (
            <div className="flex items-center justify-center py-12 text-slate-500 border border-white/5 rounded-xl bg-slate-800/20">
                <svg className="animate-spin -ml-1 mr-3 h-5 w-5 text-indigo-400" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                </svg>
                <span>Loading data...</span>
            </div>
        );
    }
    return (
        <div className="overflow-x-auto rounded-xl border border-white/5">
            <table className="w-full text-sm border-collapse">
                <thead>
                    <tr className="bg-slate-800/60">
                        {columns.map(col => (
                            <th
                                key={String(col.key)}
                                className={`
                  px-4 py-3 text-xs font-semibold tracking-wider text-slate-400 uppercase whitespace-nowrap
                  border-b border-white/5
                  ${col.align === 'right' ? 'text-right' : col.align === 'center' ? 'text-center' : 'text-left'}
                `}
                                style={col.width ? { width: col.width } : undefined}
                            >
                                {col.header}
                            </th>
                        ))}
                    </tr>
                </thead>
                <tbody>
                    {data.length === 0 ? (
                        <tr>
                            <td colSpan={columns.length} className="px-4 py-12 text-center text-slate-500">
                                {emptyMessage}
                            </td>
                        </tr>
                    ) : (
                        data.map((row, idx) => (
                            <tr
                                key={String(row[keyField])}
                                onClick={() => onRowClick?.(row)}
                                className={`
                  border-b border-white/[0.04] transition-all
                  ${onRowClick ? 'cursor-pointer hover:bg-slate-700/30' : ''}
                  ${idx % 2 === 0 ? '' : 'bg-slate-800/20'}
                `}
                            >
                                {columns.map(col => (
                                    <td
                                        key={String(col.key)}
                                        className={`
                      px-4 py-3 text-slate-300
                      ${col.align === 'right' ? 'text-right' : col.align === 'center' ? 'text-center' : 'text-left'}
                    `}
                                    >
                                        {col.render
                                            ? col.render(row)
                                            : String((row as Record<string, unknown>)[String(col.key)] ?? '—')}
                                    </td>
                                ))}
                            </tr>
                        ))
                    )}
                </tbody>
            </table>
        </div>
    );
}
