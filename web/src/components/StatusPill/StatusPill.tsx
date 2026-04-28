import styled from 'styled-components';
import { color, font, radius, space } from '@/theme';

export type StatusVariant = 'ok' | 'info' | 'warn' | 'err' | 'neutral';

interface Props {
  variant: StatusVariant;
  children: React.ReactNode;
  full?: boolean; // true = pill (999px); false = tag (4px)
}

const PALETTE: Record<StatusVariant, { bg: string; fg: string }> = {
  ok:      { bg: color.tealSoft,  fg: color.tealText },
  info:    { bg: color.blueSoft,  fg: color.blueText },
  warn:    { bg: color.yellowSoft, fg: color.amberText },
  err:     { bg: color.redSoft,   fg: color.redText },
  neutral: { bg: color.bgSoft,    fg: color.text2 },
};

export default function StatusPill({ variant, children, full }: Props) {
  const { bg, fg } = PALETTE[variant];
  return (
    <Pill $bg={bg} $fg={fg} $full={!!full}>
      {children}
    </Pill>
  );
}

const Pill = styled.span<{ $bg: string; $fg: string; $full: boolean }>`
  display: inline-flex;
  align-items: center;
  font-family: ${font.ui};
  font-size: 10px;
  font-weight: 600;
  line-height: 1;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  padding: 3px ${space.px8}px;
  border-radius: ${(p) => (p.$full ? radius.pill : radius.tag)};
  background: ${(p) => p.$bg};
  color: ${(p) => p.$fg};
`;
