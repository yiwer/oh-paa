import { useQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import type { CanonicalKline, OpenBar } from '@/api/types';

export function useCanonicalKlines(
  instrumentId: string,
  timeframe: string,
  limit: number = 48,
) {
  return useQuery({
    queryKey: ['canonical-klines', instrumentId, timeframe, limit],
    queryFn: () =>
      api<{ rows: CanonicalKline[] }>(
        `/market/canonical?instrument_id=${instrumentId}&timeframe=${timeframe}&limit=${limit}&descending=true`,
      ).then((r) => r.rows.reverse()),
    enabled: !!instrumentId,
    staleTime: 15_000,
  });
}

export function useOpenBar(instrumentId: string, timeframe: string) {
  return useQuery({
    queryKey: ['open-bar', instrumentId, timeframe],
    queryFn: () =>
      api<OpenBar>(
        `/market/open-bar?instrument_id=${instrumentId}&timeframe=${timeframe}`,
      ),
    enabled: !!instrumentId,
    staleTime: 5_000,
    refetchInterval: 15_000,
  });
}
