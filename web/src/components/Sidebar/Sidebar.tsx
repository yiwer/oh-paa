import { NavLink } from 'react-router-dom';
import styled from 'styled-components';
import { color, font, border, radius, space, transition } from '@/theme';

const navItems = [
  { to: '/pipeline', label: 'Pipeline', icon: 'P' },
  { to: '/kline', label: 'K-Line Charts', icon: 'K' },
  { to: '/llm-trace', label: 'LLM Trace', icon: 'L' },
];

interface Props {
  wsConnected: boolean;
}

const Wrap = styled.aside`
  width: 196px;
  min-height: 100vh;
  background: ${color.bgSide};
  display: flex;
  flex-direction: column;
  border-right: 1px solid #E2DAC9;
`;

const Brand = styled.div`
  font-family: ${font.ui};
  font-size: 18px;
  font-weight: 700;
  color: ${color.text1};
  padding: ${space.px20}px ${space.px16}px;
  letter-spacing: -0.01em;
  display: inline-flex;
  align-items: center;
  gap: ${space.px8}px;

  &::before {
    content: '';
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: ${color.yellow};
  }
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
  padding: ${space.px8}px ${space.px10}px;
  font-family: ${font.ui};
  font-size: 13px;
  font-weight: 500;
  color: ${color.text2};
  border: 2px solid transparent;
  border-radius: ${radius.control};
  transition: ${transition.control};
  text-decoration: none;

  &:hover {
    background: rgba(26, 26, 26, 0.04);
    color: ${color.text1};
  }

  &.active {
    background: ${color.yellow};
    color: ${color.text1};
    font-weight: 600;
    border-color: ${color.text1};
  }
`;

const IconBox = styled.span`
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  background: ${color.bgSurface};
  color: ${color.text1};
  font-weight: 700;
  font-size: 12px;
  border: ${border.default};
  border-radius: ${radius.control};
`;

const Footer = styled.div`
  padding: ${space.px16}px;
  border-top: ${border.dashed};
`;

const StatusDot = styled.span<{ $connected: boolean }>`
  color: ${(p) => (p.$connected ? color.teal : color.red)};
  font-size: 10px;
`;

const StatusText = styled.span`
  font-family: ${font.mono};
  font-size: 11px;
  color: ${color.text2};
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
