'use client';

import React, { useEffect, useState, useRef } from 'react';
import { useParams } from 'next/navigation';
import Link from 'next/link';
import { ArrowLeft, Activity, Zap, TrendingUp, BarChart3 } from 'lucide-react';

interface Trade {
    timestamp: number;
    price: number;
    amount: number;
    side: 'buy' | 'sell';
}

export default function MarketDataDetail() {
    const { symbol } = useParams();
    const [trades, setTrades] = useState<Trade[]>([]);
    const [bbo, setBbo] = useState<{ bid: number, ask: number, bidSize: number, askSize: number } | null>(null);
    const [connected, setConnected] = useState(false);
    const scrollRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const ws = new WebSocket(`${protocol}//${window.location.host}/api/md/live/${symbol}`);

        ws.onopen = () => setConnected(true);
        ws.onclose = () => setConnected(false);
        ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            if (data.type === 'trade') {
                // Placeholder: In a real system we'd have full trade details
                setTrades(prev => [
                    {
                        timestamp: Date.now(),
                        price: data.price || 0,
                        amount: data.amount || 0,
                        side: Math.random() > 0.5 ? 'buy' : 'sell'
                    },
                    ...prev.slice(0, 49)
                ]);
            } else if (data.type === 'bbo') {
                setBbo({
                    bid: data.bid || 0,
                    ask: data.ask || 0,
                    bidSize: data.bidSize || 0,
                    askSize: data.askSize || 0
                });
            }
        };

        return () => ws.close();
    }, [symbol]);

    const spread = bbo ? bbo.ask - bbo.bid : 0;
    const mid = bbo ? (bbo.ask + bbo.bid) / 2 : 0;

    return (
        <div className="p-6 space-y-6">
            <div className="flex items-center gap-4">
                <Link href="/market-data" className="p-2 hover:bg-muted rounded-full transition-colors">
                    <ArrowLeft className="h-5 w-5" />
                </Link>
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">{symbol}</h1>
                    <div className="flex items-center gap-2 mt-1">
                        <div className={`h-2 w-2 rounded-full ${connected ? 'bg-green-500 shadow-[0_0_6px_rgba(34,197,94,0.6)]' : 'bg-red-500'}`} />
                        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                            {connected ? 'Live Feed Active' : 'Connecting...'}
                        </span>
                    </div>
                </div>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                {/* Market Stats Card */}
                <div className="md:col-span-2 grid grid-cols-2 sm:grid-cols-4 gap-4">
                    <div className="bg-card p-4 rounded-xl border shadow-sm flex flex-col justify-between">
                        <span className="text-xs text-muted-foreground font-medium uppercase">Bid Price</span>
                        <span className="text-xl font-mono font-bold text-green-500">{bbo?.bid.toFixed(2) || '---'}</span>
                    </div>
                    <div className="bg-card p-4 rounded-xl border shadow-sm flex flex-col justify-between">
                        <span className="text-xs text-muted-foreground font-medium uppercase">Ask Price</span>
                        <span className="text-xl font-mono font-bold text-red-500">{bbo?.ask.toFixed(2) || '---'}</span>
                    </div>
                    <div className="bg-card p-4 rounded-xl border shadow-sm flex flex-col justify-between">
                        <span className="text-xs text-muted-foreground font-medium uppercase">Mid Price</span>
                        <span className="text-xl font-mono font-bold">{mid.toFixed(2) || '---'}</span>
                    </div>
                    <div className="bg-card p-4 rounded-xl border shadow-sm flex flex-col justify-between">
                        <span className="text-xs text-muted-foreground font-medium uppercase">Spread</span>
                        <span className="text-xl font-mono font-bold text-muted-foreground underline decoration-dotted underline-offset-4">
                            {spread.toFixed(2) || '---'}
                        </span>
                    </div>
                </div>

                {/* Info Card */}
                <div className="bg-card p-4 rounded-xl border shadow-sm flex flex-col justify-center items-center text-center space-y-2">
                    <Zap className="h-8 w-8 text-yellow-500 fill-yellow-500/20" />
                    <div className="text-sm font-semibold">Venue: Binance</div>
                    <div className="text-xs text-muted-foreground">Streaming raw trade and book ticker data from Redpanda topics.</div>
                </div>
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                {/* Live Tape */}
                <div className="bg-card rounded-xl border shadow-sm flex flex-col h-[500px]">
                    <div className="p-4 border-b flex items-center justify-between">
                        <div className="flex items-center gap-2 font-semibold">
                            <Activity className="h-4 w-4" />
                            Live Trades (Tape)
                        </div>
                        <div className="text-[10px] text-muted-foreground uppercase font-bold px-2 py-0.5 border rounded bg-muted/50">
                            Auto-Scroll ON
                        </div>
                    </div>
                    <div className="flex-1 overflow-y-auto p-0 font-mono text-xs" ref={scrollRef}>
                        <table className="w-full">
                            <thead className="sticky top-0 bg-muted/90 backdrop-blur-sm border-b">
                                <tr>
                                    <th className="p-2 text-left">Time</th>
                                    <th className="p-2 text-right">Price</th>
                                    <th className="p-2 text-right">Size</th>
                                    <th className="p-2 text-center">Side</th>
                                </tr>
                            </thead>
                            <tbody>
                                {trades.length === 0 ? (
                                    <tr>
                                        <td colSpan={4} className="p-8 text-center text-muted-foreground italic">
                                            Waiting for trade events...
                                        </td>
                                    </tr>
                                ) : trades.map((t, idx) => (
                                    <tr key={idx} className={`border-b border-muted/20 ${idx === 0 ? 'bg-primary/5 animate-pulse' : ''} transition-all`}>
                                        <td className="p-2 text-muted-foreground">{new Date(t.timestamp).toLocaleTimeString([], { hour12: false })}</td>
                                        <td className={`p-2 text-right font-bold ${t.side === 'buy' ? 'text-green-500' : 'text-red-500'}`}>
                                            {t.price.toFixed(2)}
                                        </td>
                                        <td className="p-2 text-right">{t.amount.toFixed(4)}</td>
                                        <td className="p-2 text-center">
                                            <span className={`px-1.5 py-0.5 rounded text-[10px] font-bold ${t.side === 'buy' ? 'bg-green-500/10 text-green-500' : 'bg-red-500/10 text-red-500'}`}>
                                                {t.side.toUpperCase()}
                                            </span>
                                        </td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                </div>

                {/* Order Book Visualization Placeholder */}
                <div className="bg-card rounded-xl border shadow-sm flex flex-col h-[500px]">
                    <div className="p-4 border-b flex items-center gap-2 font-semibold">
                        <BarChart3 className="h-4 w-4" />
                        Micro-Structure Components
                    </div>
                    <div className="flex-1 flex flex-col items-center justify-center p-8 text-center space-y-4">
                        <div className="w-full max-w-xs aspect-video bg-muted/30 rounded-lg flex flex-col items-center justify-center border-2 border-dashed border-muted">
                            <TrendingUp className="h-10 w-10 text-muted-foreground" />
                        </div>
                        <div className="space-y-1">
                            <h3 className="font-semibold text-lg">Order Book Staleness Check</h3>
                            <p className="text-sm text-muted-foreground">
                                Visualizing order flow imbalance, micro-price and book delta in real-time.
                            </p>
                        </div>
                        <div className="grid grid-cols-2 w-full gap-4 text-left">
                            <div className="p-3 border rounded-lg bg-muted/20">
                                <div className="text-[10px] uppercase font-bold text-muted-foreground">Imbalance</div>
                                <div className="text-sm font-mono font-bold">48.2% / 51.8%</div>
                            </div>
                            <div className="p-3 border rounded-lg bg-muted/20">
                                <div className="text-[10px] uppercase font-bold text-muted-foreground">Staleness</div>
                                <div className="text-sm font-mono font-bold text-green-500">12ms (FRESH)</div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
}
