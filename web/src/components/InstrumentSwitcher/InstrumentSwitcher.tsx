import styled from 'styled-components';
import { color, font, space, border } from '@/theme';
import type { Instrument } from '@/api/types';

interface Props {
  instruments: Instrument[];
  selectedId: string;
  onSelect: (id: string) => void;
}

function shortSymbol(symbol: string) {
  return symbol.split('/')[0];
}

export default function InstrumentSwitcher({
  instruments,
  selectedId,
  onSelect,
}: Props) {
  const crypto = instruments.filter((i) => i.symbol.includes('/USDT'));
  const forex = instruments.filter((i) => !i.symbol.includes('/USDT'));

  return (
    <Row>
      {crypto.map((i) => (
        <Pill
          key={i.id}
          $active={i.id === selectedId}
          onClick={() => onSelect(i.id)}
        >
          {shortSymbol(i.symbol)}
        </Pill>
      ))}

      {crypto.length > 0 && forex.length > 0 && <Sep>|</Sep>}

      {forex.map((i) => (
        <Pill
          key={i.id}
          $active={i.id === selectedId}
          onClick={() => onSelect(i.id)}
        >
          {shortSymbol(i.symbol)}
        </Pill>
      ))}
    </Row>
  );
}

/* ---- styled ---- */

const Row = styled.div`
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: ${space.px4}px;
`;

const Pill = styled.button<{ $active: boolean }>`
  all: unset;
  cursor: pointer;
  font-family: ${font.mono};
  font-size: 11px;
  font-weight: 700;
  padding: ${space.px4}px ${space.px8}px;
  border: ${border.std};
  border-radius: 0px;
  background: ${(p) => (p.$active ? color.textDark : color.bgWhite)};
  color: ${(p) => (p.$active ? color.yellowPrimary : color.textDark)};
  transition: background-color 0.15s, color 0.15s;

  &:hover {
    background: ${(p) =>
      p.$active ? color.textDark : color.bgLightGray};
  }
`;

const Sep = styled.span`
  color: ${color.textLightGray};
  font-size: 11px;
  user-select: none;
  padding: 0 ${space.px4}px;
`;
