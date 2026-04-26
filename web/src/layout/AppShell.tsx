import { Outlet } from 'react-router-dom';
import styled from 'styled-components';
import Sidebar from '@/components/Sidebar/Sidebar';
import { color } from '@/theme';
import { useWebSocket } from '@/ws/useWebSocket';
import { useDebugEventStore } from '@/ws/debugEventStore';

const Page = styled.div`
  display: flex;
  min-height: 100vh;
  background: ${color.bgBeige};
`;

const Main = styled.main`
  flex: 1;
  padding: 28px 36px;
  max-width: 1440px;
`;

export default function AppShell() {
  useWebSocket();
  const wsConnected = useDebugEventStore((s) => s.connected);

  return (
    <Page>
      <Sidebar wsConnected={wsConnected} />
      <Main>
        <Outlet />
      </Main>
    </Page>
  );
}
