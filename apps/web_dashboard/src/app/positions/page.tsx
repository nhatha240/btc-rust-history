"use client";

import { useState, useEffect } from 'react';
import useSWR from 'swr';
import { fetchPositions, closePosition, partialClosePosition } from '@/lib/api';
import { Position } from '@/lib/types';
import { Activity, XCircle, TrendingDown, TrendingUp, AlertTriangle } from 'lucide-react';

export default function PositionsPage() {
    const { data: positions, error, mutate } = useSWR<Position[]>('positions', () => fetchPositions(), { refreshInterval: 5000 });
    const [livePrices, setLivePrices] = useState<Record<string, number>>({});
    const [closingSymbols, setClosingSymbols] = useState<Set<string>>(new Set());
    const [partialQty, setPartialQty] = useState<Record<string, string>>({});

    useEffect(() => {
        if (!positions || positions.length === 0) return;

        const wsConnections: Record<string, WebSocket> = {};

        positions.forEach(pos => {
            if (wsConnections[pos.symbol]) return;

            const wsUrl = `ws://${window.location.host}/api/md/live/${pos.symbol}`;
            const ws = new WebSocket(wsUrl);

            ws.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    if (data.type === 'bbo') {
                        const midPrice = (parseFloat(data.payload.bid_price) + parseFloat(data.payload.ask_price)) / 2;
                        setLivePrices(prev => ({ ...prev, [pos.symbol]: midPrice }));
                    } else if (data.type === 'trade') {
                        setLivePrices(prev => ({ ...prev, [pos.symbol]: parseFloat(data.payload.price) }));
                    }
                } catch (err) {
                    console.error('WebSocket parse error:', err);
                }
            };

            wsConnections[pos.symbol] = ws;
        });

        return () => {
            Object.values(wsConnections).forEach(ws => ws.close());
        };
    }, [positions]);

    const handleClose = async (symbol: string) => {
        if (!confirm(`Are you sure you want to Market Close the entire position for ${symbol}?`)) return;

        setClosingSymbols(prev => new Set(prev).add(symbol));
        try {
            await closePosition(symbol);
            mutate();
        } catch (err) {
            alert(`Failed to close position: ${err}`);
        } finally {
            setClosingSymbols(prev => {
                const next = new Set(prev);
                next.delete(symbol);
                return next;
            });
        }
    };

    const handlePartialClose = async (symbol: string) => {
        const qtyToClose = partialQty[symbol];
        if (!qtyToClose || isNaN(parseFloat(qtyToClose)) || parseFloat(qtyToClose) <= 0) {
            alert("Enter a valid quantity to close.");
            return;
        }

        if (!confirm(`Are you sure you want to Market Close ${qtyToClose} of ${symbol}?`)) return;

        setClosingSymbols(prev => new Set(prev).add(symbol));
        try {
            await partialClosePosition(symbol, qtyToClose);
            mutate();
            setPartialQty(prev => ({ ...prev, [symbol]: "" }));
        } catch (err) {
            alert(`Failed to partially close position: ${err}`);
        } finally {
            setClosingSymbols(prev => {
                const next = new Set(prev);
                next.delete(symbol);
                return next;
            });
        }
    };

    const calculateLiveUnrealizedPnL = (pos: Position) => {
        const markPrice = livePrices[pos.symbol];
        if (!markPrice || !pos.entry_price) return null;

        const entry = parseFloat(pos.entry_price);
        const qty = parseFloat(pos.qty);
        const isLong = pos.side === 'LONG';

        // Simple linear PnL formula calculation (adjust depending on linear/inverse futures specs)
        const pnl = isLong
            ? (markPrice - entry) * qty
            : (entry - markPrice) * qty;

        return pnl;
    };

    const calculateLiveROE = (pos: Position, livePnl: number | null) => {
        if (livePnl === null || !pos.entry_price) return null;
        const initialMargin = (parseFloat(pos.entry_price) * parseFloat(pos.qty)) / pos.leverage;
        if (initialMargin === 0) return 0;
        return (livePnl / initialMargin) * 100;
    };

    if (error) return <div className="p-8 text-red-400">Error loading positions: {error.toString()}</div>;
    if (!positions) return <div className="p-8 text-gray-400">Loading positions...</div>;

    return (
        <div className="p-8 max-w-7xl mx-auto space-y-6">
            <div className="flex justify-between items-center bg-gray-900/40 p-6 rounded-2xl border border-gray-800">
                <div>
                    <h1 className="text-3xl font-bold bg-gradient-to-r from-teal-400 to-emerald-400 bg-clip-text text-transparent flex items-center gap-3">
                        <Activity className="w-8 h-8 text-teal-400" />
                        Position Management
                    </h1>
                    <p className="text-gray-400 mt-2">Real-time exposure monitoring and risk controls.</p>
                </div>
                <div className="flex gap-4">
                    <button
                        className="px-4 py-2 bg-red-500/10 text-red-400 border border-red-500/20 hover:bg-red-500/20 rounded-lg flex items-center gap-2 transition-colors font-medium"
                        onClick={() => {
                            if (confirm("PANIC CLOSE ALL POSITIONS? This will submit market orders for every open position.")) {
                                positions.forEach(p => handleClose(p.symbol));
                            }
                        }}
                    >
                        <AlertTriangle className="w-4 h-4" />
                        Panic Close All
                    </button>
                </div>
            </div>

            <div className="grid gap-6">
                {positions.length === 0 ? (
                    <div className="text-center py-20 bg-gray-900/40 rounded-2xl border border-gray-800/50">
                        <p className="text-gray-400">No open positions.</p>
                    </div>
                ) : (
                    positions.map((pos) => {
                        const isLong = pos.side === 'LONG';
                        const livePnl = calculateLiveUnrealizedPnL(pos);
                        const liveRoe = calculateLiveROE(pos, livePnl);

                        const pnlColor = livePnl && livePnl >= 0 ? 'text-green-400' : 'text-red-400';
                        const reqClosing = closingSymbols.has(pos.symbol);

                        return (
                            <div key={pos.id} className="bg-gray-900/60 border border-gray-800 rounded-2xl p-6 transition-all hover:border-gray-700/80 w-full">
                                <div className="flex flex-col lg:flex-row justify-between gap-6">

                                    {/* Info Column */}
                                    <div className="flex-1 space-y-4">
                                        <div className="flex items-center gap-4">
                                            <div className={`px-3 py-1 rounded-md text-sm font-bold tracking-wider ${isLong ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400'}`}>
                                                {isLong ? 'LONG' : 'SHORT'}
                                            </div>
                                            <h2 className="text-2xl font-bold text-white">{pos.symbol}</h2>
                                            <span className="text-xs px-2 py-1 rounded-full bg-blue-500/10 text-blue-400 border border-blue-500/20">
                                                {pos.leverage}x {pos.margin_type}
                                            </span>
                                        </div>

                                        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
                                            <div className="bg-gray-800/30 p-3 rounded-xl border border-gray-800/80">
                                                <p className="text-xs text-gray-500 uppercase font-semibold">Size</p>
                                                <p className="font-medium text-gray-200">{parseFloat(pos.qty).toFixed(4)}</p>
                                            </div>
                                            <div className="bg-gray-800/30 p-3 rounded-xl border border-gray-800/80">
                                                <p className="text-xs text-gray-500 uppercase font-semibold">Avg Entry</p>
                                                <p className="font-medium text-gray-200">${parseFloat(pos.entry_price || '0').toFixed(2)}</p>
                                            </div>
                                            <div className="bg-gray-800/30 p-3 rounded-xl border border-gray-800/80">
                                                <div className="flex items-center gap-2">
                                                    <p className="text-xs text-gray-500 uppercase font-semibold">Mark Price</p>
                                                    <span className="relative flex h-2 w-2">
                                                        <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-teal-400 opacity-75"></span>
                                                        <span className="relative inline-flex rounded-full h-2 w-2 bg-teal-500"></span>
                                                    </span>
                                                </div>
                                                <p className="font-medium text-emerald-300">
                                                    {livePrices[pos.symbol] ? `$${livePrices[pos.symbol].toFixed(2)}` : 'Waiting for tick...'}
                                                </p>
                                            </div>
                                            <div className="bg-gray-800/30 p-3 rounded-xl border border-gray-800/80">
                                                <p className="text-xs text-yellow-600/60 uppercase font-semibold">Liq. Price</p>
                                                <p className="font-medium text-yellow-500">
                                                    {pos.liquidation_price ? `$${parseFloat(pos.liquidation_price).toFixed(2)}` : 'N/A'}
                                                </p>
                                            </div>
                                        </div>
                                    </div>

                                    {/* PNL Column */}
                                    <div className="flex flex-col justify-center min-w-[200px] border-l border-gray-800 pl-6">
                                        <p className="text-xs text-gray-500 uppercase font-semibold mb-1">Unrealized PNL</p>
                                        <div className={`text-3xl font-bold flex items-center gap-2 ${pnlColor}`}>
                                            {livePnl !== null ? (
                                                <>
                                                    {livePnl >= 0 ? <TrendingUp className="w-6 h-6" /> : <TrendingDown className="w-6 h-6" />}
                                                    ${Math.abs(livePnl).toFixed(2)}
                                                </>
                                            ) : (
                                                <span className="text-gray-600">--</span>
                                            )}
                                        </div>
                                        {liveRoe !== null && (
                                            <p className={`text-sm font-medium mt-1 ${pnlColor}`}>
                                                {liveRoe >= 0 ? '+' : ''}{liveRoe.toFixed(2)}% ROE
                                            </p>
                                        )}
                                    </div>

                                    {/* Actions Column */}
                                    <div className="flex flex-col justify-center gap-3 border-l border-gray-800 pl-6 w-full lg:w-48">
                                        <button
                                            className="w-full py-2 bg-rose-500 hover:bg-rose-600 text-white font-medium rounded-lg flex items-center justify-center gap-2 transition-all disabled:opacity-50"
                                            onClick={() => handleClose(pos.symbol)}
                                            disabled={reqClosing}
                                        >
                                            {reqClosing ? (
                                                <span className="animate-pulse">Closing...</span>
                                            ) : (
                                                <>
                                                    <XCircle className="w-4 h-4" /> Market Close
                                                </>
                                            )}
                                        </button>

                                        <div className="flex flex-col gap-1">
                                            <label className="text-xs text-gray-500 font-medium">Partial Close Qty:</label>
                                            <div className="flex gap-2">
                                                <input
                                                    type="text"
                                                    placeholder="0.01"
                                                    className="w-full bg-gray-950 border border-gray-800 rounded-md px-2 text-sm text-gray-200 focus:outline-none focus:border-rose-500/50"
                                                    value={partialQty[pos.symbol] || ''}
                                                    onChange={(e) => setPartialQty(prev => ({ ...prev, [pos.symbol]: e.target.value }))}
                                                    disabled={reqClosing}
                                                />
                                                <button
                                                    className="px-3 py-1 bg-rose-500/20 hover:bg-rose-500/40 text-rose-400 font-medium rounded-md text-sm transition-colors disabled:opacity-50"
                                                    onClick={() => handlePartialClose(pos.symbol)}
                                                    disabled={reqClosing}
                                                >
                                                    Close
                                                </button>
                                            </div>
                                        </div>
                                    </div>

                                </div>
                            </div>
                        );
                    })
                )}
            </div>
        </div>
    );
}
