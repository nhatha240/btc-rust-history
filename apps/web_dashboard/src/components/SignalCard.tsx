'use client';

import { format } from 'date-fns';

interface SignalCardProps {
  signal: {
    symbol: string;
    ts: number;
    side: string;
    strategy_name: string;
    price: number;
    confidence: number;
    reason: string;
  };
}

export default function SignalCard({ signal }: SignalCardProps) {
  const isLong = signal.side === 'LONG';
  const timestamp = new Date(signal.ts);

  return (
    <div className={`rounded-lg p-4 border-2 transition-all duration-300 hover:shadow-lg ${
      isLong ? 'border-green-200 bg-green-50' : 'border-red-200 bg-red-50'
    }`}
    >
      <div className="flex justify-between items-start mb-2">
        <h3 className="text-xl font-bold">{signal.symbol}</h3>
        <span className={`px-2 py-1 text-xs font-semibold rounded-full ${
          isLong ? 'bg-green-100 text-green-800' : 'bg-red-100 text-red-800'
        }`}
        >
          {signal.side}
        </span>
      </div>

      <div className="mb-3">
        <p className="text-sm text-gray-600">{signal.reason}</p>
      </div>

      <div className="grid grid-cols-2 gap-2 mb-3 text-sm">
        <div>
          <span className="text-gray-500">Price:</span>
          <span className="ml-1 font-medium">${signal.price.toFixed(2)}</span>
        </div>
        <div>
          <span className="text-gray-500">Confidence:</span>
          <span className="ml-1 font-medium">{Math.round(signal.confidence * 100)}%</span>
        </div>
        <div className="col-span-2">
          <span className="text-gray-500">Strategy:</span>
          <span className="ml-1 font-medium">{signal.strategy_name}</span>
        </div>
      </div>

      <div className="text-xs text-gray-500">
        {format(timestamp, 'MMM d, HH:mm:ss')}
      </div>
    </div>
  );
}