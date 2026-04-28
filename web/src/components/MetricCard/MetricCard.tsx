import styled, { keyframes } from 'styled-components';
import { color, font, border, radius, shadow, space } from '@/theme';

export type MetricAccent = 'teal' | 'blue' | 'yellow' | 'red' | 'gray';

const accentColors: Record<MetricAccent, string> = {
  teal: color.teal,
  blue: color.blue,
  yellow: color.yellow,
  red: color.red,
  gray: color.text3,
};

interface Props {
  accent?: MetricAccent;
  eyebrow: string;
  value: React.ReactNode;
  sub?: string;
}

export default function MetricCard({ accent = 'gray', eyebrow, value, sub }: Props) {
  return (
    <Card>
      <AccentBar $accent={accent} />
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

const Card = styled.div`
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: ${radius.card};
  box-shadow: ${shadow.card};
  padding: ${space.px12}px ${space.px16}px;
  position: relative;
  flex: 1;
`;

const AccentBar = styled.span<{ $accent: MetricAccent }>`
  display: block;
  width: 24px;
  height: 3px;
  border-radius: 2px;
  background: ${(p) => accentColors[p.$accent]};
  margin-bottom: ${space.px8}px;
`;

const Eyebrow = styled.div`
  font-family: ${font.ui};
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: ${color.text3};
  margin-bottom: 2px;
`;

const Value = styled.div`
  font-family: ${font.mono};
  font-size: 22px;
  font-weight: 600;
  color: ${color.text1};
  line-height: 1.2;
`;

const Sub = styled.div`
  font-family: ${font.ui};
  font-size: 11px;
  color: ${color.text2};
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
  gap: ${space.px12}px;

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
