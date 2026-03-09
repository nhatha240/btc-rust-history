'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { cancelOrder } from '@/lib/api';

interface Props {
    clientOrderId: string;
    status: string;
}

export function CancelOrderButton({ clientOrderId, status }: Props) {
    const router = useRouter();
    const [isCanceling, setIsCanceling] = useState(false);

    if (!['NEW', 'PARTIALLY_FILLED', 'PartiallyFilled', 'New'].includes(status)) {
        return null;
    }

    const handleCancel = async () => {
        setIsCanceling(true);
        try {
            await cancelOrder(clientOrderId);
            // Refresh Server Component payload
            router.refresh();
        } catch (error) {
            console.error('Cancel failed', error);
            alert('Failed to cancel order');
            setIsCanceling(false);
        }
    };

    return (
        <button
            onClick={handleCancel}
            disabled={isCanceling}
            className="bg-rose-500 hover:bg-rose-600 active:bg-rose-700 text-white font-medium px-4 py-2 rounded shadow-lg shadow-rose-900/20 transition-all text-sm disabled:opacity-50 flex items-center gap-2"
        >
            {isCanceling ? (
                <>
                    <svg className="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
                        <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                        <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8z" />
                    </svg>
                    Canceling...
                </>
            ) : (
                'Cancel Order'
            )}
        </button>
    );
}
