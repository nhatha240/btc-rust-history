'use client';

const RANGES = [
    { label: '1h', value: 1 },
    { label: '6h', value: 6 },
    { label: '24h', value: 24 },
    { label: '7d', value: 24 * 7 },
    { label: 'All', value: 0 },
];

interface Props {
    value: number;  // hours; 0 = all
    onChange: (hours: number) => void;
}

export function TimeRangePicker({ value, onChange }: Props) {
    return (
        <div className="flex items-center gap-1 bg-slate-800/50 border border-white/10 rounded-lg p-1">
            {RANGES.map(r => (
                <button
                    key={r.label}
                    onClick={() => onChange(r.value)}
                    className={`
            px-3 py-1 rounded-md text-sm font-medium transition-all
            ${value === r.value
                            ? 'bg-indigo-600 text-white shadow-sm shadow-indigo-900'
                            : 'text-slate-400 hover:text-slate-200 hover:bg-slate-700'}
          `}
                >
                    {r.label}
                </button>
            ))}
        </div>
    );
}
