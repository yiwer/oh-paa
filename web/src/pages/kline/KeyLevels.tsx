import styled from 'styled-components';
import { color, font, border, radius, shadow, space } from '@/theme';
import type { CanonicalKline, KeyLevel } from '@/api/types';

interface Props {
  kline?: CanonicalKline;
  keyLevels: KeyLevel[];
}

interface LevelDistance {
  level: KeyLevel;
  distancePct: number;
}

function nearestLevels(price: number, levels: KeyLevel[]): LevelDistance[] {
  if (!levels.length) return [];
  return levels
    .map((l) => ({ level: l, distancePct: ((Number(l.price) - price) / price) * 100 }))
    .sort((a, b) => Math.abs(a.distancePct) - Math.abs(b.distancePct))
    .slice(0, 6);
}

function fmtPct(pct: number): string {
  const sign = pct > 0 ? '+' : '';
  return `${sign}${pct.toFixed(2)}%`;
}

export default function KeyLevels({ kline, keyLevels }: Props) {
  if (!kline) {
    return (
      <Root>
        <Title>Key Levels</Title>
        <EmptyHint>select a bar to inspect</EmptyHint>
      </Root>
    );
  }

  const close = Number(kline.close);
  const ranked = nearestLevels(close, keyLevels);

  return (
    <Root>
      <Title>Key Levels</Title>
      {ranked.length === 0 ? (
        <EmptyHint>no key levels for this bar</EmptyHint>
      ) : (
        <List>
          {ranked.map((r, i) => (
            <Row key={i}>
              <KindDot $type={r.level.type} />
              <Kind>{r.level.type}</Kind>
              <Price>{r.level.price}</Price>
              <Distance $above={r.distancePct >= 0}>{fmtPct(r.distancePct)}</Distance>
            </Row>
          ))}
        </List>
      )}
    </Root>
  );
}

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

const List = styled.div`
  display: flex;
  flex-direction: column;
`;

const Row = styled.div`
  display: grid;
  grid-template-columns: 12px 1fr auto auto;
  align-items: center;
  gap: ${space.px8}px;
  padding: 4px 0;
  font-family: ${font.mono};
  font-size: 11px;
  border-bottom: 1px dashed ${color.borderSoft};

  &:last-child {
    border-bottom: none;
  }
`;

const KindDot = styled.span<{ $type: KeyLevel['type'] }>`
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: ${(p) =>
    p.$type === 'support' ? color.teal : p.$type === 'resistance' ? color.red : color.yellow};
`;

const Kind = styled.span`
  font-family: ${font.ui};
  font-size: 11px;
  color: ${color.text2};
  text-transform: capitalize;
`;

const Price = styled.span`
  font-weight: 600;
  color: ${color.text1};
`;

const Distance = styled.span<{ $above: boolean }>`
  color: ${(p) => (p.$above ? color.tealText : color.redText)};
  font-weight: 500;
  min-width: 56px;
  text-align: right;
`;
