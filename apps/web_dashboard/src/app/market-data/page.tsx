'use client';

import React, { useEffect, useState } from 'react';
import { fetchMdHealth } from '@/lib/api';
import { VenueHealth } from '@/lib/types';
import Link from 'next/link';

export default function MarketDataPage() {
    const [data, setData] = useState<VenueHealth[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        const load = async () => {
            try {
                const health = await fetchMdHealth();
                setData(health);
                setLoading(false);
            } catch (e: any) {
                setError(e.message);
                setLoading(false);
            }
        };

        load();
        const timer = setInterval(load, 2000);
        return () => clearInterval(timer);
    }, []);

    if (loading) return <div className="p-8">Loading market data health...</div>;
    if (error) return <div className="p-8 text-red-500">Error: {error}</div>;

    return (
        <div className="p-6 space-y-6">
            <div className="flex justify-between items-end">
                <div>
                    <h1 className="text-3xl font-bold tracking-tight">Market Data Management</h1>
                    <p className="text-muted-foreground mt-1">Real-time feed health and connectivity status.</p>
                </div>
            </div>

            {data.map((venue) => (
                <div key={venue.venue} className="bg-card rounded-lg border shadow-sm">
                    <div className="p-4 border-b flex justify-between items-center bg-muted/30">
                        <div className="flex items-center gap-3">
                            <h2 className="text-xl font-semibold capitalize">{venue.venue}</h2>
                            <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-green-500/10 text-green-500 border border-green-500/20">
                                CONNECTED
                            </span>
                        </div>
                        <div className="text-sm text-muted-foreground">
                            Total Reconnects: <span className="text-foreground font-mono">{venue.reconnects}</span>
                        </div>
                    </div>

                    <div className="overflow-x-auto">
                        <table className="w-full text-left text-sm">
                            <thead>
                                <tr className="border-b bg-muted/10">
                                    <th className="p-4 font-medium">Symbol</th>
                                    <th className="p-4 font-medium text-right">Msg Rate</th>
                                    <th className="p-4 font-medium text-right">Latency</th>
                                    <th className="p-4 font-medium text-right">Last Message</th>
                                    <th className="p-4 font-medium text-center">Status</th>
                                    <th className="p-4 font-medium">Actions</th>
                                </tr>
                            </thead>
                            <tbody>
                                {venue.symbols.map((s) => (
                                    <tr key={s.symbol} className="border-b hover:bg-muted/5 transition-colors">
                                        <td className="p-4 font-semibold">{s.symbol}</td>
                                        <td className="p-4 text-right font-mono">{s.msg_rate}/s</td>
                                        <td className="p-4 text-right font-mono">
                                            <span className={s.latency_ms > 100 ? 'text-yellow-500' : 'text-green-500'}>
                                                {s.latency_ms.toFixed(1)}ms
                                            </span>
                                        </td>
                                        <td className="p-4 text-right text-muted-foreground font-mono">
                                            {s.last_msg_ts > 0 ? new Date(s.last_msg_ts / 1000000).toLocaleTimeString() : 'N/A'}
                                        </td>
                                        <td className="p-4 text-center">
                                            <div className="flex justify-center">
                                                <div className={`h-2.5 w-2.5 rounded-full ${s.msg_rate > 0 ? 'bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]' : 'bg-red-500'}`} />
                                            </div>
                                        </td>
                                        <td className="p-4">
                                            <Link
                                                href={`/market-data/${s.symbol}`}
                                                className="text-xs font-medium text-primary hover:underline"
                                            >
                                                Details
                                            </Link>
                                        </td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                </div>
            ))}
        </div>
    );
}
