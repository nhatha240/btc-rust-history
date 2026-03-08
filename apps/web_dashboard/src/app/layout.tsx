import type { Metadata } from 'next';
import './globals.css';
import Link from 'next/link';

export const metadata: Metadata = {
    title: 'HFT Trading Dashboard',
    description: 'Order history, trades, positions and PnL for your HFT bot',
};

const NAV_ITEMS = [
    { href: '/orders', icon: '📋', label: 'Orders' },
    { href: '/trades', icon: '⚡', label: 'Trades' },
    { href: '/positions', icon: '📊', label: 'Positions' },
    { href: '/pnl', icon: '💰', label: 'PnL' },
    { href: '/verification', icon: '🔍', label: 'Verification' },
    { href: '/coins', icon: '🪙', label: 'Coins' },
];

export default function RootLayout({ children }: { children: React.ReactNode }) {
    return (
        <html lang="en" className="dark">
            <body className="flex h-screen overflow-hidden bg-[#0f172a] text-slate-200">
                {/* Sidebar */}
                <aside className="w-56 flex-shrink-0 flex flex-col bg-[#1a2335] border-r border-white/5">
                    {/* Logo */}
                    <div className="px-5 py-5 border-b border-white/5">
                        <div className="flex items-center gap-2.5">
                            <span className="text-2xl">🤖</span>
                            <div>
                                <p className="font-semibold text-sm text-slate-100 leading-4">HFT Bot</p>
                                <p className="text-[10px] text-indigo-400 font-medium tracking-wider uppercase mt-0.5">Dashboard</p>
                            </div>
                        </div>
                    </div>

                    {/* Nav */}
                    <nav className="flex-1 px-3 py-4 space-y-0.5 overflow-y-auto">
                        {NAV_ITEMS.map(item => (
                            <Link
                                key={item.href}
                                href={item.href}
                                className="flex items-center gap-3 px-3 py-2.5 rounded-lg text-slate-400 hover:bg-slate-700/50 hover:text-slate-100 transition-all text-sm font-medium group"
                            >
                                <span className="text-base w-5 text-center">{item.icon}</span>
                                {item.label}
                            </Link>
                        ))}
                    </nav>

                    {/* Footer status */}
                    <div className="px-4 py-3 border-t border-white/5">
                        <div className="flex items-center gap-2 text-xs text-slate-500">
                            <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                            Live
                        </div>
                    </div>
                </aside>

                {/* Main */}
                <main className="flex-1 overflow-y-auto">
                    {children}
                </main>
            </body>
        </html>
    );
}
