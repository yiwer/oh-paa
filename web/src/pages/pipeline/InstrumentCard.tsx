import { useState } from 'react';
import { Link } from 'react-router-dom';
import styled from 'styled-components';
import { color, font, border, radius, shadow, space } from '@/theme';
import type { Instrument, DebugEvent } from '@/api/types';

interface Props {
  instrument: Instrument;
  events: DebugEvent[];
  hasError: boolean;
}

export default function InstrumentCard({ instrument, events, hasError }: Props) {
  const [expanded, setExpanded] = useState(false);
  const recentEvents = events.slice(-5);

  return (
    <Card $hasError={hasError} onClick={() => setExpanded((v) => !v)}>
      <Header>
        <Left>
          <SymbolLink
            to={`/kline?instrument=${instrument.id}`}
            onClick={(e) => e.stopPropagation()}
          >
            {instrument.symbol}
          </SymbolLink>
          <Name>{instrument.name}</Name>
        </Left>
        <Arrow $expanded={expanded}>{'\u25B6'}</Arrow>
      </Header>

      {expanded && (
        <ExpandedSection>
          <SectionTitle>Recent Events</SectionTitle>
          {recentEvents.length === 0 ? (
            <EmptyLabel>No events yet</EmptyLabel>
          ) : (
            recentEvents.map((ev, i) => (
              <EventRow key={i}>
                <EventType>{String(ev.type)}</EventType>
                <EventDetail>
                  {ev.instrument_id
                    ? String(ev.instrument_id).slice(0, 8)
                    : '\u2014'}
                </EventDetail>
                <EventDetail>
                  {ev.timeframe ? String(ev.timeframe) : ''}
                </EventDetail>
                <EventDetail>
                  {ev.provider ? String(ev.provider) : ''}
                </EventDetail>
                <EventDetail>
                  {ev.latency_ms != null ? `${ev.latency_ms}ms` : ''}
                </EventDetail>
              </EventRow>
            ))
          )}
        </ExpandedSection>
      )}
    </Card>
  );
}

/* ---- styled ---- */

const Card = styled.div<{ $hasError: boolean }>`
  position: relative;
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: ${radius.card};
  box-shadow: ${shadow.card};
  padding: ${space.px12}px ${space.px16}px ${space.px12}px ${space.px20}px;
  cursor: pointer;
  user-select: none;
  overflow: hidden;

  &::before {
    content: '';
    position: absolute;
    left: 0;
    top: 12px;
    bottom: 12px;
    width: 3px;
    border-radius: 0 2px 2px 0;
    background: ${(p) => (p.$hasError ? color.red : color.teal)};
  }
`;

const Header = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
`;

const Left = styled.div`
  display: flex;
  align-items: baseline;
  gap: ${space.px8}px;
`;

const SymbolLink = styled(Link)`
  font-family: ${font.mono};
  font-size: 14px;
  font-weight: 700;
  color: ${color.blueText};
  text-decoration: none;
  cursor: pointer;
`;

const Name = styled.span`
  font-family: ${font.ui};
  font-size: 13px;
  color: ${color.text2};
`;

const Arrow = styled.span<{ $expanded: boolean }>`
  font-size: 10px;
  color: ${color.text3};
  transition: transform 0.15s ease;
  transform: rotate(${(p) => (p.$expanded ? '90deg' : '0deg')});
`;

const ExpandedSection = styled.div`
  margin-top: ${space.px10}px;
  padding-top: ${space.px10}px;
  border-top: ${border.dashed};
`;

const SectionTitle = styled.div`
  font-family: ${font.ui};
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: ${color.text3};
  margin-bottom: ${space.px6}px;
`;

const EmptyLabel = styled.div`
  font-family: ${font.ui};
  font-size: 13px;
  color: ${color.text3};
`;

const EventRow = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px8}px;
  padding: 3px 0;
  font-family: ${font.mono};
  font-size: 11px;
  border-bottom: 1px dashed ${color.borderSoft};

  &:last-child {
    border-bottom: none;
  }
`;

const EventType = styled.span`
  font-weight: 600;
  color: ${color.text1};
  min-width: 120px;
`;

const EventDetail = styled.span`
  color: ${color.text2};
`;
