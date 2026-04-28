import { useEffect, useRef, useState } from 'react';
import styled from 'styled-components';
import { color, font, border, radius, shadow, space, transition } from '@/theme';
import type { Instrument } from '@/api/types';

interface Props {
  instruments: Instrument[];
  selectedId: string;
  onSelect: (id: string) => void;
  /** Optional label shown to the left of the trigger. */
  label?: string;
}

function isCrypto(symbol: string) {
  return symbol.includes('/USDT');
}

function shortSymbol(symbol: string) {
  return symbol.split('/')[0];
}

export default function InstrumentDropdown({ instruments, selectedId, onSelect, label }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', onDoc);
    return () => document.removeEventListener('mousedown', onDoc);
  }, [open]);

  const selected = instruments.find((i) => i.id === selectedId);
  const crypto = instruments.filter((i) => isCrypto(i.symbol));
  const forex = instruments.filter((i) => !isCrypto(i.symbol));

  return (
    <Wrap ref={ref}>
      {label && <Label>{label}</Label>}
      <Trigger type="button" onClick={() => setOpen((v) => !v)}>
        <TriggerSymbol>{selected ? shortSymbol(selected.symbol) : '—'}</TriggerSymbol>
        <Caret>{'▾'}</Caret>
      </Trigger>
      {open && (
        <Panel>
          {crypto.length > 0 && (
            <Group>
              <GroupTitle>Crypto</GroupTitle>
              {crypto.map((i) => (
                <Item
                  key={i.id}
                  $selected={i.id === selectedId}
                  onClick={() => {
                    onSelect(i.id);
                    setOpen(false);
                  }}
                >
                  <ItemSymbol>{shortSymbol(i.symbol)}</ItemSymbol>
                  <ItemMeta>{i.name}</ItemMeta>
                </Item>
              ))}
            </Group>
          )}
          {forex.length > 0 && (
            <Group>
              <GroupTitle>Forex</GroupTitle>
              {forex.map((i) => (
                <Item
                  key={i.id}
                  $selected={i.id === selectedId}
                  onClick={() => {
                    onSelect(i.id);
                    setOpen(false);
                  }}
                >
                  <ItemSymbol>{shortSymbol(i.symbol)}</ItemSymbol>
                  <ItemMeta>{i.name}</ItemMeta>
                </Item>
              ))}
            </Group>
          )}
        </Panel>
      )}
    </Wrap>
  );
}

const Wrap = styled.div`
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: ${space.px8}px;
`;

const Label = styled.span`
  font-family: ${font.ui};
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: ${color.text3};
`;

const Trigger = styled.button`
  all: unset;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: ${space.px8}px;
  min-width: 150px;
  padding: 6px ${space.px10}px;
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: ${radius.control};
  box-shadow: ${shadow.card};
  font-family: ${font.mono};
  font-size: 13px;
  font-weight: 600;
  color: ${color.text1};
  transition: ${transition.control};

  &:hover {
    background: ${color.bgSoft};
  }
`;

const TriggerSymbol = styled.span`
  flex: 1;
`;

const Caret = styled.span`
  font-size: 10px;
  color: ${color.text2};
`;

const Panel = styled.div`
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  min-width: 220px;
  z-index: 50;
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: 8px;
  box-shadow: ${shadow.popover};
  padding: ${space.px6}px;
`;

const Group = styled.div`
  & + & {
    margin-top: ${space.px6}px;
    padding-top: ${space.px6}px;
    border-top: ${border.dashed};
  }
`;

const GroupTitle = styled.div`
  font-family: ${font.ui};
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: ${color.text3};
  padding: ${space.px4}px ${space.px8}px;
`;

const Item = styled.div<{ $selected: boolean }>`
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: ${space.px8}px;
  padding: 7px ${space.px10}px;
  border-radius: 4px;
  font-family: ${font.mono};
  font-size: 13px;
  cursor: pointer;
  background: ${(p) => (p.$selected ? color.yellowSoft : 'transparent')};
  font-weight: ${(p) => (p.$selected ? 600 : 500)};
  color: ${color.text1};

  &:hover {
    background: ${(p) => (p.$selected ? color.yellowSoft : color.bgSoft)};
  }
`;

const ItemSymbol = styled.span`
  font-weight: inherit;
`;

const ItemMeta = styled.span`
  font-family: ${font.ui};
  font-size: 11px;
  color: ${color.text3};
`;
