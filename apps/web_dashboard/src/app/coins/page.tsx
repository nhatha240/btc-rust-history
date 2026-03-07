'use client';
import { useState } from 'react';
import useSWR from 'swr';

const fetcher = (url: string) => fetch(url).then(r => r.json());

interface Coin {
    id: number;
    symbol: string;
    base_asset: string;
    quote_asset: string;
    is_active: boolean;
    created_at: string;
    updated_at: string;
}

export default function CoinsPage() {
    const { data: coins, error, mutate } = useSWR<Coin[]>('http://localhost:8088/api/coins', fetcher);
    const [isAdding, setIsAdding] = useState(false);
    
    // Add form state
    const [symbol, setSymbol] = useState('');
    const [baseAsset, setBaseAsset] = useState('');
    const [quoteAsset, setQuoteAsset] = useState('');
    const [errorMsg, setErrorMsg] = useState('');

    const handleAdd = async (e: React.FormEvent) => {
        e.preventDefault();
        setErrorMsg('');
        try {
            const res = await fetch('http://localhost:8088/api/coins', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    symbol,
                    base_asset: baseAsset,
                    quote_asset: quoteAsset,
                    is_active: true
                })
            });
            const data = await res.json();
            if (data.status === 'ok') {
                setIsAdding(false);
                setSymbol('');
                setBaseAsset('');
                setQuoteAsset('');
                mutate(); // Refresh data
            } else {
                setErrorMsg(data.message || 'Error adding coin');
            }
        } catch (err) {
            setErrorMsg('Network error');
        }
    };

    const toggleActive = async (coin: Coin) => {
        try {
            const res = await fetch(`http://localhost:8088/api/coins/${coin.symbol}`, {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ is_active: !coin.is_active })
            });
            if (res.ok) {
                mutate();
            }
        } catch (err) {
            console.error('Failed to toggle active state', err);
        }
    };

    const deleteCoin = async (coinSymbol: string) => {
        if (!confirm(`Are you sure you want to delete ${coinSymbol}?`)) return;
        try {
            const res = await fetch(`http://localhost:8088/api/coins/${coinSymbol}`, {
                method: 'DELETE'
            });
            if (res.ok) {
                mutate();
            }
        } catch (err) {
            console.error('Failed to delete coin', err);
        }
    };

    return (
        <div className="p-8 space-y-6">
            <div className="flex items-center justify-between border-b border-white/5 pb-5">
                <div>
                    <h1 className="text-2xl font-semibold text-slate-100 mb-1 tracking-tight">Coin Management</h1>
                    <p className="text-slate-400 text-sm">Manage tradable symbols and their status.</p>
                </div>
                <button 
                    onClick={() => setIsAdding(!isAdding)}
                    className="bg-indigo-500 hover:bg-indigo-600 text-white px-4 py-2 rounded-md font-medium text-sm transition-colors"
                >
                    {isAdding ? 'Cancel' : 'Add Coin'}
                </button>
            </div>

            {isAdding && (
                <div className="bg-[#1a2335] p-6 rounded-lg border border-white/5 space-y-4">
                    <h2 className="text-lg font-medium text-slate-200">Add New Coin</h2>
                    {errorMsg && <div className="text-red-400 text-sm">{errorMsg}</div>}
                    <form onSubmit={handleAdd} className="flex gap-4 items-end">
                        <div className="flex-1 space-y-1">
                            <label className="text-xs text-slate-400 font-medium ml-1">Symbol</label>
                            <input 
                                value={symbol} onChange={e => setSymbol(e.target.value)} required
                                placeholder="BTCUSDT"
                                className="w-full bg-[#0f172a] border border-white/10 rounded-md px-3 py-2 text-sm text-slate-200 outline-none focus:border-indigo-500" 
                            />
                        </div>
                        <div className="flex-1 space-y-1">
                            <label className="text-xs text-slate-400 font-medium ml-1">Base Asset</label>
                            <input 
                                value={baseAsset} onChange={e => setBaseAsset(e.target.value)} required
                                placeholder="BTC"
                                className="w-full bg-[#0f172a] border border-white/10 rounded-md px-3 py-2 text-sm text-slate-200 outline-none focus:border-indigo-500" 
                            />
                        </div>
                        <div className="flex-1 space-y-1">
                            <label className="text-xs text-slate-400 font-medium ml-1">Quote Asset</label>
                            <input 
                                value={quoteAsset} onChange={e => setQuoteAsset(e.target.value)} required
                                placeholder="USDT"
                                className="w-full bg-[#0f172a] border border-white/10 rounded-md px-3 py-2 text-sm text-slate-200 outline-none focus:border-indigo-500" 
                            />
                        </div>
                        <button type="submit" className="bg-emerald-500 hover:bg-emerald-600 text-white px-6 py-2 rounded-md font-medium text-sm transition-colors mb-[1px]">
                            Save
                        </button>
                    </form>
                </div>
            )}

            <div className="bg-[#1a2335] border border-white/5 rounded-xl overflow-hidden">
                <table className="w-full text-left text-sm">
                    <thead>
                        <tr className="border-b border-white/5 bg-white/[0.02]">
                            <th className="px-6 py-4 font-medium text-slate-400">Symbol</th>
                            <th className="px-6 py-4 font-medium text-slate-400">Asset Pair</th>
                            <th className="px-6 py-4 font-medium text-slate-400 text-center">Status</th>
                            <th className="px-6 py-4 font-medium text-slate-400 text-right">Actions</th>
                        </tr>
                    </thead>
                    <tbody className="divide-y divide-white/5">
                        {error && (
                            <tr><td colSpan={4} className="px-6 py-8 text-center text-red-400">Error loading coins</td></tr>
                        )}
                        {!coins && !error && (
                            <tr><td colSpan={4} className="px-6 py-8 text-center text-slate-400">Loading...</td></tr>
                        )}
                        {coins?.length === 0 && (
                            <tr><td colSpan={4} className="px-6 py-8 text-center text-slate-500">No coins configured yet</td></tr>
                        )}
                        {coins?.map(coin => (
                            <tr key={coin.symbol} className="hover:bg-white/[0.02] transition-colors">
                                <td className="px-6 py-4 font-medium text-slate-200">{coin.symbol}</td>
                                <td className="px-6 py-4 text-slate-400">
                                    <span className="text-slate-300">{coin.base_asset}</span>
                                    <span className="text-slate-500 mx-1">/</span>
                                    <span>{coin.quote_asset}</span>
                                </td>
                                <td className="px-6 py-4 text-center">
                                    <button 
                                        onClick={() => toggleActive(coin)}
                                        className={`px-3 py-1 rounded-full text-xs font-medium border transition-colors ${
                                            coin.is_active 
                                            ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20 hover:bg-emerald-500/20' 
                                            : 'bg-red-500/10 text-red-400 border-red-500/20 hover:bg-red-500/20'
                                        }`}
                                    >
                                        {coin.is_active ? 'Active' : 'Inactive'}
                                    </button>
                                </td>
                                <td className="px-6 py-4 text-right space-x-3">
                                    <button 
                                        onClick={() => deleteCoin(coin.symbol)}
                                        className="text-slate-500 hover:text-red-400 transition-colors"
                                    >
                                        Delete
                                    </button>
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            </div>
        </div>
    );
}
