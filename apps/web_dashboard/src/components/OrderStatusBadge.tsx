'use client';

import { OrderStatus } from '@/lib/types';
import { statusColor } from '@/lib/format';

interface Props {
    status: OrderStatus;
    showDot?: boolean;
}

export function OrderStatusBadge({ status, showDot = true }: Props) {
    const { bg, text, dot } = statusColor(status);
    return (
        <span className={`inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-xs font-semibold tracking-wide ${bg} ${text}`}>
            {showDot && <span className={`w-1.5 h-1.5 rounded-full ${dot}`} />}
            {status.replace('_', ' ')}
        </span>
    );
}
