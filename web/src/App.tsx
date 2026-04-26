import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import AppShell from '@/layout/AppShell';

const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 30_000, refetchOnWindowFocus: false } },
});

function PlaceholderPage({ title }: { title: string }) {
  return <h2 style={{ fontSize: 24, fontWeight: 700 }}>{title}</h2>;
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route element={<AppShell />}>
            <Route path="/" element={<Navigate to="/pipeline" replace />} />
            <Route path="/pipeline" element={<PlaceholderPage title="Pipeline" />} />
            <Route path="/kline" element={<PlaceholderPage title="K-Line Charts" />} />
            <Route path="/llm-trace" element={<PlaceholderPage title="LLM Trace" />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
