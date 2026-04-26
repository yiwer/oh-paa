import { Outlet } from 'react-router-dom';
import styled from 'styled-components';
import Sidebar from '@/components/Sidebar/Sidebar';
import { color } from '@/theme';

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
  const wsConnected = false; // placeholder, wired in Task 10

  return (
    <Page>
      <Sidebar wsConnected={wsConnected} />
      <Main>
        <Outlet />
      </Main>
    </Page>
  );
}
