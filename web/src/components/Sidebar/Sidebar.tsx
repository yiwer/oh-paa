import { NavLink } from 'react-router-dom';
import styled from 'styled-components';
import { color, font, border, space, transition } from '@/theme';

const navItems = [
  { to: '/pipeline', label: 'Pipeline', icon: 'P' },
  { to: '/kline', label: 'K-Line Charts', icon: 'K' },
  { to: '/llm-trace', label: 'LLM Trace', icon: 'L' },
];

interface Props {
  wsConnected: boolean;
}

const Wrap = styled.aside`
  width: 200px;
  min-height: 100vh;
  background: ${color.darkSurface};
  display: flex;
  flex-direction: column;
  border-right: ${border.std};
`;

const Brand = styled.div`
  font-family: ${font.mono};
  font-size: 20px;
  font-weight: 800;
  color: ${color.yellowPrimary};
  text-transform: uppercase;
  padding: ${space.px20}px ${space.px16}px;
  letter-spacing: 2px;
`;

const Nav = styled.nav`
  display: flex;
  flex-direction: column;
  gap: ${space.px4}px;
  padding: ${space.px8}px;
  flex: 1;
`;

const StyledNavLink = styled(NavLink)`
  display: flex;
  align-items: center;
  gap: ${space.px10}px;
  padding: ${space.px10}px ${space.px12}px;
  font-family: ${font.mono};
  font-size: 13px;
  font-weight: 500;
  color: ${color.bgBeige};
  border: 2px solid transparent;
  border-radius: ${border.radius};
  transition: ${transition.nav};
  text-decoration: none;

  &:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  &.active {
    background: ${color.yellowPrimary};
    color: ${color.textDark};
    font-weight: 700;
    border-color: ${color.textDark};
  }
`;

const IconBox = styled.span`
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  background: ${color.bgWhite};
  color: ${color.textDark};
  font-weight: 700;
  font-size: 12px;
  border: ${border.std};
  border-radius: ${border.radius};
`;

const Footer = styled.div`
  padding: ${space.px16}px;
  border-top: 1px solid rgba(255, 255, 255, 0.12);
`;

const StatusDot = styled.span<{ $connected: boolean }>`
  color: ${(p) => (p.$connected ? color.tealAccent : color.redAccent)};
  font-size: 10px;
`;

const StatusText = styled.span`
  font-family: ${font.mono};
  font-size: 11px;
  color: ${color.textLightGray};
`;

export default function Sidebar({ wsConnected }: Props) {
  return (
    <Wrap>
      <Brand>oh-paa</Brand>
      <Nav>
        {navItems.map((item) => (
          <StyledNavLink key={item.to} to={item.to}>
            <IconBox>{item.icon}</IconBox>
            {item.label}
          </StyledNavLink>
        ))}
      </Nav>
      <Footer>
        <StatusDot $connected={wsConnected}>&bull;</StatusDot>{' '}
        <StatusText>{wsConnected ? 'Connected' : 'Disconnected'}</StatusText>
      </Footer>
    </Wrap>
  );
}
