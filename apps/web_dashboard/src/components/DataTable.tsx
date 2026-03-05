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
}

export function DataTable<T>({ columns, data, keyField, onRowClick, emptyMessage = 'No data' }: Props<T>) {
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
