import { useQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import type { Instrument, SessionProfile } from '@/api/types';

// Note: the backend may not have a /market/instruments endpoint yet.
// For now, create the hook structure. If the endpoint doesn't exist,
// we'll use mock data or disable the query.

export function useInstruments() {
  return useQuery({
    queryKey: ['instruments'],
    queryFn: () => api<{ rows: Instrument[] }>('/market/instruments').then(r => r.rows),
    staleTime: 60_000,
  });
}

export function useSessionProfile(instrumentId: string) {
  return useQuery({
    queryKey: ['session-profile', instrumentId],
    queryFn: () => api<SessionProfile>(`/market/session-profile?instrument_id=${instrumentId}`),
    enabled: !!instrumentId,
    staleTime: 300_000,
  });
}
