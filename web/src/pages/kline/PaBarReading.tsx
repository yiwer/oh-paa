import styled from 'styled-components';
import { color, font, border, radius, shadow, space } from '@/theme';
import type { BarReading } from '@/api/types';

interface Props {
  barReading?: BarReading;
}

const COLOR_MAP: Record<string, string> = {
  green: color.teal,
  red: color.red,
  gray: color.text3,
  yellow: color.yellow,
};

const BIAS_COLOR: Record<string, string> = {
  bullish: color.teal,
  bearish: color.red,
  neutral: color.text3,
};

function fmtTime(iso: string): string {
  const d = new Date(iso);
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  const hh = String(d.getHours()).padStart(2, '0');
  const mi = String(d.getMinutes()).padStart(2, '0');
  return `${mm}-${dd} ${hh}:${mi}`;
}

export default function PaBarReading({ barReading }: Props) {
  if (!barReading) {
    return (
      <Root>
        <Title>PA Bar Reading</Title>
        <EmptyHint>click a bar to inspect</EmptyHint>
      </Root>
    );
  }

  const dotColor = COLOR_MAP[barReading.bar_reading_color] ?? color.textGray;
  const biasColor = BIAS_COLOR[barReading.bias.toLowerCase()] ?? color.textGray;

  return (
    <Root>
      <Title>PA Bar Reading</Title>
      <ContentArea>
        <Header>
          <ColorDot style={{ background: dotColor }} />
          <PatternName>{barReading.pattern}</PatternName>
          <BarTime>{fmtTime(barReading.bar_close_time)}</BarTime>
        </Header>
        <Summary>{barReading.bar_summary}</Summary>
        <Footer>
          <Tag>Structure: {barReading.structure}</Tag>
          <TagSep>|</TagSep>
          <Tag>
            Bias: <span style={{ color: biasColor, fontWeight: 700 }}>{barReading.bias}</span>
          </Tag>
          <TagSep>|</TagSep>
          <Tag>Source: {barReading.source}</Tag>
        </Footer>
      </ContentArea>
    </Root>
  );
}

/* ---- styled ---- */

const Root = styled.div`
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: ${radius.card};
  box-shadow: ${shadow.card};
  padding: ${space.px12}px ${space.px16}px;
  font-family: ${font.ui};
  min-height: 140px;
  display: flex;
  flex-direction: column;
`;

const Title = styled.h4`
  font-family: ${font.ui};
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: ${color.text1};
  margin: 0 0 ${space.px8}px 0;
`;

const EmptyHint = styled.span`
  font-size: 13px;
  color: ${color.text3};
`;

const ContentArea = styled.div`
  padding-top: ${space.px8}px;
  border-top: ${border.dashed};
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: ${space.px8}px;
`;

const Header = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px8}px;
`;

const ColorDot = styled.span`
  display: inline-block;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
`;

const PatternName = styled.span`
  font-family: ${font.ui};
  font-size: 14px;
  font-weight: 700;
  color: ${color.text1};
`;

const BarTime = styled.span`
  font-family: ${font.mono};
  font-size: 11px;
  color: ${color.text3};
  margin-left: auto;
`;

const Summary = styled.p`
  font-family: ${font.ui};
  font-size: 13px;
  color: ${color.text1};
  line-height: 1.5;
  margin: 0;
`;

const Footer = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px6}px;
  margin-top: auto;
  padding-top: ${space.px8}px;
  border-top: ${border.dashed};
`;

const Tag = styled.span`
  font-family: ${font.mono};
  font-size: 11px;
  color: ${color.text2};
`;

const TagSep = styled.span`
  font-size: 11px;
  color: ${color.textDisabled};
  user-select: none;
`;
