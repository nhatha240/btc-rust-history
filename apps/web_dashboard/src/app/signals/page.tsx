import { useState, useEffect } from 'react';
import { fetchSignals } from '@/lib/api';
import SignalCard from '@/components/SignalCard';

export default function SignalsPage() {
  const [signals, setSignals] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  useEffect(() => {
    const loadData = async () => {
      try {
        const data = await fetchSignals();
        setSignals(data);
      } catch (err) {
        setError('Failed to load signals');
        console.error(err);
      } finally {
        setLoading(false);
      }
    };
    loadData();
  }, []);

  if (loading) return <div>Loading signals...</div>;
  if (error) return <div>Error: {error}</div>;

  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-2xl font-bold mb-6">Trading Signals Dashboard</h1>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {signals.map((signal) => (
          <SignalCard key={signal.symbol} signal={signal} />
        ))}
      </div>
    </div>
  );
}