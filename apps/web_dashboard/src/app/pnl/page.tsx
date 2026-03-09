"use client";

import { useMemo } from "react";
import useSWR from "swr";
import { fetchTrades } from "@/lib/api";
import { Trade } from "@/lib/types";

type SeriesPoint = {
  label: string;
  value: number;
};

type DailyPoint = {
  date: string;
  pnl: number;
};

type PerformanceMetrics = {
  totalPnl: number;
  sharpeRatio: number;
  maxDrawdownPct: number;
  avgDailyPnl: number;
};

function toDayKey(iso: string): string {
  return new Date(iso).toISOString().slice(0, 10);
}

function formatCompact(n: number, digits = 2): string {
  return n.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: digits,
  });
}

function formatPercent(n: number, digits = 2): string {
  return `${n.toFixed(digits)}%`;
}

function buildPerformance(trades: Trade[]): {
  metrics: PerformanceMetrics;
  equityCurve: SeriesPoint[];
  drawdownCurve: SeriesPoint[];
  dailyPnl: DailyPoint[];
} {
  const pnlByDay = new Map<string, number>();

  for (const trade of trades) {
    const day = toDayKey(trade.trade_time);
    const pnl = Number.parseFloat(trade.realized_pnl ?? "0") || 0;
    pnlByDay.set(day, (pnlByDay.get(day) ?? 0) + pnl);
  }

  const dailyPnl = Array.from(pnlByDay.entries())
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([date, pnl]) => ({ date, pnl }));

  let equity = 0;
  let peak = 0;
  const equityCurve: SeriesPoint[] = [];
  const drawdownCurve: SeriesPoint[] = [];

  for (const d of dailyPnl) {
    equity += d.pnl;
    peak = Math.max(peak, equity);

    const ddPct = peak > 0 ? ((equity - peak) / peak) * 100 : 0;
    equityCurve.push({ label: d.date, value: equity });
    drawdownCurve.push({ label: d.date, value: ddPct });
  }

  const totalPnl = equity;
  const avgDailyPnl = dailyPnl.length > 0 ? totalPnl / dailyPnl.length : 0;
  const maxDrawdownPct = drawdownCurve.reduce((min, p) => Math.min(min, p.value), 0);

  // Daily-return Sharpe proxy with a synthetic base equity to avoid division by zero.
  const baseEquity = 10_000;
  const returns = dailyPnl.map((d, idx) => {
    const prevEquity = idx === 0 ? baseEquity : baseEquity + equityCurve[idx - 1].value;
    return d.pnl / Math.max(prevEquity, 1);
  });

  const mean = returns.length > 0 ? returns.reduce((a, b) => a + b, 0) / returns.length : 0;
  const variance =
    returns.length > 1
      ? returns.reduce((acc, r) => acc + (r - mean) ** 2, 0) / (returns.length - 1)
      : 0;
  const std = Math.sqrt(variance);
  const sharpeRatio = std > 0 ? (mean / std) * Math.sqrt(365) : 0;

  return {
    metrics: {
      totalPnl,
      sharpeRatio,
      maxDrawdownPct,
      avgDailyPnl,
    },
    equityCurve,
    drawdownCurve,
    dailyPnl,
  };
}

function LineChart({
  points,
  stroke,
  minY,
  maxY,
}: {
  points: SeriesPoint[];
  stroke: string;
  minY?: number;
  maxY?: number;
}) {
  const width = 100;
  const height = 30;

  if (points.length === 0) {
    return <div className="text-sm text-slate-500">No data</div>;
  }

  const values = points.map((p) => p.value);
  const lo = minY ?? Math.min(...values);
  const hi = maxY ?? Math.max(...values);
  const range = hi - lo || 1;

  const path = points
    .map((p, i) => {
      const x = (i / Math.max(points.length - 1, 1)) * width;
      const y = height - ((p.value - lo) / range) * height;
      return `${i === 0 ? "M" : "L"}${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(" ");

  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="w-full h-56">
      <path d={path} fill="none" stroke={stroke} strokeWidth="1.8" />
    </svg>
  );
}

function DailyPnlBars({ points }: { points: DailyPoint[] }) {
  const width = 100;
  const height = 30;

  if (points.length === 0) {
    return <div className="text-sm text-slate-500">No data</div>;
  }

  const values = points.map((p) => p.pnl);
  const absMax = Math.max(...values.map((v) => Math.abs(v)), 1);
  const baselineY = height / 2;
  const barWidth = width / points.length;

  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="w-full h-56">
      <line x1="0" y1={baselineY} x2={width} y2={baselineY} stroke="#475569" strokeWidth="0.6" />
      {points.map((p, i) => {
        const scaled = (Math.abs(p.pnl) / absMax) * (height / 2);
        const x = i * barWidth + barWidth * 0.1;
        const w = barWidth * 0.8;
        const y = p.pnl >= 0 ? baselineY - scaled : baselineY;
        return (
          <rect
            key={p.date}
            x={x}
            y={y}
            width={w}
            height={scaled}
            fill={p.pnl >= 0 ? "#34d399" : "#f87171"}
            opacity="0.9"
          />
        );
      })}
    </svg>
  );
}

export default function PnlPage() {
  const { data: trades = [], isLoading, error } = useSWR(["trades", "pnl-analytics"], () =>
    fetchTrades({ limit: 2000 })
  );

  const perf = useMemo(() => buildPerformance(trades), [trades]);

  if (isLoading) {
    return <div className="p-8 text-slate-400">Loading performance analytics...</div>;
  }

  if (error) {
    return <div className="p-8 text-rose-400">Failed to load performance data.</div>;
  }

  return (
    <div className="p-8 space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Analytics / Performance</h1>
        <p className="text-slate-400 text-sm mt-1">
          Equity curve, Sharpe ratio, daily PnL, and drawdown from realized trade history.
        </p>
      </div>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        {[
          {
            label: "Total Realized PnL",
            value: `$${formatCompact(perf.metrics.totalPnl)}`,
            color: perf.metrics.totalPnl >= 0 ? "text-emerald-400" : "text-rose-400",
          },
          {
            label: "Sharpe Ratio",
            value: perf.metrics.sharpeRatio.toFixed(2),
            color: perf.metrics.sharpeRatio >= 0 ? "text-sky-400" : "text-rose-400",
          },
          {
            label: "Max Drawdown",
            value: formatPercent(perf.metrics.maxDrawdownPct),
            color: "text-amber-400",
          },
          {
            label: "Avg Daily PnL",
            value: `$${formatCompact(perf.metrics.avgDailyPnl)}`,
            color: perf.metrics.avgDailyPnl >= 0 ? "text-emerald-400" : "text-rose-400",
          },
        ].map((k) => (
          <div key={k.label} className="card px-5 py-4">
            <p className="text-xs text-slate-500 uppercase tracking-wider font-semibold">{k.label}</p>
            <p className={`text-2xl font-bold mt-1 ${k.color}`}>{k.value}</p>
          </div>
        ))}
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
        <div className="card p-5">
          <h3 className="text-base font-semibold text-slate-100 mb-3">Equity Curve</h3>
          <LineChart points={perf.equityCurve} stroke="#22d3ee" />
        </div>

        <div className="card p-5">
          <h3 className="text-base font-semibold text-slate-100 mb-3">Drawdown Curve (%)</h3>
          <LineChart points={perf.drawdownCurve} stroke="#f59e0b" maxY={0} />
        </div>
      </div>

      <div className="card p-5">
        <h3 className="text-base font-semibold text-slate-100 mb-3">Daily PnL</h3>
        <DailyPnlBars points={perf.dailyPnl} />
      </div>
    </div>
  );
}
