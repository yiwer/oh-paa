import styled, { keyframes } from 'styled-components';
import { color, border, space } from '@/theme';

export type MetricAccent = 'teal' | 'blue' | 'yellow' | 'red' | 'gray';

const accentColors: Record<MetricAccent, string> = {
  teal: color.tealAccent,
  blue: color.bluePrimary,
  yellow: color.yellowPrimary,
  red: color.redAccent,
  gray: color.textGray,
};

interface Props {
  accent?: MetricAccent;
  eyebrow: string;
  value: React.ReactNode;
  sub?: string;
}

export default function MetricCard({ accent = 'gray', eyebrow, value, sub }: Props) {
  return (
    <Card $accent={accent}>
      <Eyebrow>{eyebrow}</Eyebrow>
      <Value>{value}</Value>
      {sub && <Sub>{sub}</Sub>}
    </Card>
  );
}

export function MetricStrip({ children, ...rest }: React.HTMLAttributes<HTMLDivElement>) {
  return <Strip {...rest}>{children}</Strip>;
}

/* ---- styled ---- */

const Card = styled.div<{ $accent: MetricAccent }>`
  background: ${color.bgWhite};
  border: ${border.std};
  border-left: 4px solid ${(p) => accentColors[p.$accent]};
  padding: ${space.px10}px ${space.px12}px;
`;

const Eyebrow = styled.div`
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.8px;
  color: ${color.textGray};
  margin-bottom: 2px;
`;

const Value = styled.div`
  font-size: 22px;
  font-weight: 800;
  color: ${color.textDark};
  line-height: 1.2;
`;

const Sub = styled.div`
  font-size: 11px;
  color: ${color.textGray};
  margin-top: 2px;
`;

const fadeIn = keyframes`
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
`;

const Strip = styled.div`
  display: flex;
  flex-direction: row;
  gap: ${space.px10}px;

  & > * {
    animation: ${fadeIn} 0.3s ease-out both;
  }
  & > *:nth-child(1) { animation-delay: 0ms; }
  & > *:nth-child(2) { animation-delay: 60ms; }
  & > *:nth-child(3) { animation-delay: 120ms; }
  & > *:nth-child(4) { animation-delay: 180ms; }
  & > *:nth-child(5) { animation-delay: 240ms; }
  & > *:nth-child(6) { animation-delay: 300ms; }
`;
