import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import AppShell from '@/layout/AppShell';
import PipelinePage from '@/pages/PipelinePage';
import KLinePage from '@/pages/KLinePage';
import LlmTracePage from '@/pages/LlmTracePage';
import TaskDetailPage from '@/pages/TaskDetailPage';

const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 30_000, refetchOnWindowFocus: false } },
});

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route element={<AppShell />}>
            <Route path="/" element={<Navigate to="/pipeline" replace />} />
            <Route path="/pipeline" element={<PipelinePage />} />
            <Route path="/kline" element={<KLinePage />} />
            <Route path="/llm-trace" element={<LlmTracePage />} />
            <Route path="/llm-trace/:taskId" element={<TaskDetailPage />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
