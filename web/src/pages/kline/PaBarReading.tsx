import styled from 'styled-components';
import { color, font, size, space, border } from '@/theme';
import type { BarReading } from '@/api/types';

interface Props {
  barReading?: BarReading;
}

const COLOR_MAP: Record<string, string> = {
  green: color.tealAccent,
  red: color.redAccent,
  gray: color.textGray,
  yellow: color.yellowPrimary,
};

const BIAS_COLOR: Record<string, string> = {
  bullish: color.tealAccent,
  bearish: color.redAccent,
  neutral: color.textGray,
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
  background: ${color.bgWhite};
  border: ${border.std};
  padding: ${space.px16}px;
  font-family: ${font.mono};
  min-height: 140px;
  display: flex;
  flex-direction: column;
`;

const Title = styled.h4`
  font-size: ${size.eyebrow}px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 1px;
  color: ${color.textGray};
  margin: 0 0 ${space.px8}px 0;
`;

const EmptyHint = styled.span`
  font-size: ${size.bodySm}px;
  color: ${color.textLightGray};
`;

const ContentArea = styled.div`
  border: ${border.dashedSection};
  padding: ${space.px12}px;
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
  flex-shrink: 0;
`;

const PatternName = styled.span`
  font-size: ${size.body}px;
  font-weight: 700;
  color: ${color.textDark};
`;

const BarTime = styled.span`
  font-size: ${size.caption}px;
  color: ${color.textLightGray};
  margin-left: auto;
`;

const Summary = styled.p`
  font-size: ${size.bodySm}px;
  color: ${color.textDark};
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
  font-size: ${size.caption}px;
  color: ${color.textGray};
`;

const TagSep = styled.span`
  font-size: ${size.caption}px;
  color: ${color.textLightGray};
  user-select: none;
`;
