import { useState } from 'react';
import styled from 'styled-components';
import { color, border, space, size } from '@/theme';
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
          <Symbol>{instrument.symbol}</Symbol>
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
  background: ${color.bgWhite};
  border: 2px solid ${(p) => (p.$hasError ? color.redAccent : color.textDark)};
  padding: ${space.px12}px ${space.px16}px;
  cursor: pointer;
  user-select: none;
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

const Symbol = styled.span`
  font-size: ${size.h3}px;
  font-weight: 700;
  color: ${color.textDark};
`;

const Name = styled.span`
  font-size: ${size.bodySm}px;
  color: ${color.textGray};
`;

const Arrow = styled.span<{ $expanded: boolean }>`
  font-size: 10px;
  color: ${color.textGray};
  transition: transform 0.15s ease;
  transform: rotate(${(p) => (p.$expanded ? '90deg' : '0deg')});
`;

const ExpandedSection = styled.div`
  margin-top: ${space.px10}px;
  padding-top: ${space.px10}px;
  border-top: ${border.dashedSection};
`;

const SectionTitle = styled.div`
  font-size: ${size.caption}px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.8px;
  color: ${color.textGray};
  margin-bottom: ${space.px6}px;
`;

const EmptyLabel = styled.div`
  font-size: ${size.bodySm}px;
  color: ${color.textLightGray};
`;

const EventRow = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px8}px;
  padding: 2px 0;
  font-size: ${size.bodyXs}px;
`;

const EventType = styled.span`
  font-weight: 600;
  color: ${color.textDark};
  min-width: 120px;
`;

const EventDetail = styled.span`
  color: ${color.textGray};
`;
