import { useQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import type { AnalysisTask, AnalysisAttempt, AnalysisResult, AnalysisDeadLetter } from '@/api/types';

export function useTasks(instrumentId: string) {
  return useQuery({
    queryKey: ['tasks', instrumentId],
    queryFn: () => api<{ rows: AnalysisTask[] }>(
      `/analysis/tasks?instrument_id=${instrumentId}`
    ).then(r => r.rows),
    enabled: !!instrumentId,
    staleTime: 10_000,
  });
}

export function useTask(taskId: string) {
  return useQuery({
    queryKey: ['task', taskId],
    queryFn: () => api<AnalysisTask>(`/analysis/tasks/${taskId}`),
    enabled: !!taskId,
  });
}

export function useAttempts(taskId: string) {
  return useQuery({
    queryKey: ['attempts', taskId],
    queryFn: () => api<{ rows: AnalysisAttempt[] }>(
      `/analysis/tasks/${taskId}/attempts`
    ).then(r => r.rows),
    enabled: !!taskId,
  });
}

export function useResult(taskId: string) {
  return useQuery({
    queryKey: ['result', taskId],
    queryFn: () => api<AnalysisResult>(`/analysis/results/${taskId}`),
    enabled: !!taskId,
  });
}

export function useDeadLetter(taskId: string) {
  return useQuery({
    queryKey: ['dead-letter', taskId],
    queryFn: () => api<AnalysisDeadLetter>(`/analysis/dead-letters/${taskId}`),
    enabled: !!taskId,
  });
}
