import { useState, useRef, useEffect } from 'react';
import styled from 'styled-components';
import { color, border, space, size } from '@/theme';
import type { DebugEvent, DebugEventType } from '@/api/types';

interface Props {
  events: DebugEvent[];
}

const dotColors: Record<DebugEventType, string> = {
  kline_ingested: color.tealAccent,
  provider_fallback: color.yellowPrimary,
  normalization_result: color.redAccent,
  task_status_changed: color.bluePrimary,
  attempt_completed: color.bluePrimary,
  open_bar_update: color.tealAccent,
};

function dotColor(type: string): string {
  return dotColors[type as DebugEventType] ?? color.textGray;
}

export default function EventStream({ events }: Props) {
  const [collapsed, setCollapsed] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const tail = events.slice(-50);

  useEffect(() => {
    const el = scrollRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [tail.length]);

  return (
    <Wrapper>
      <PanelHeader onClick={() => setCollapsed((v) => !v)}>
        <PanelTitle>Live Event Stream</PanelTitle>
        <Toggle>{collapsed ? '\u25B6' : '\u25BC'}</Toggle>
      </PanelHeader>

      {!collapsed && (
        <Body ref={scrollRef}>
          {tail.length === 0 ? (
            <Waiting>Waiting for events...</Waiting>
          ) : (
            tail.map((ev, i) => (
              <Row key={i}>
                <Dot $color={dotColor(String(ev.type))} />
                <TypeLabel>{String(ev.type)}</TypeLabel>
                <Detail>
                  {ev.instrument_id
                    ? String(ev.instrument_id).slice(0, 8)
                    : ''}
                </Detail>
                <Detail>{ev.timeframe ? String(ev.timeframe) : ''}</Detail>
                <Detail>{ev.provider ? String(ev.provider) : ''}</Detail>
                <Detail>
                  {ev.latency_ms != null ? `${ev.latency_ms}ms` : ''}
                </Detail>
              </Row>
            ))
          )}
        </Body>
      )}
    </Wrapper>
  );
}

/* ---- styled ---- */

const Wrapper = styled.div`
  background: ${color.bgWhite};
  border: ${border.std};
`;

const PanelHeader = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: ${space.px10}px ${space.px16}px;
  cursor: pointer;
  user-select: none;
`;

const PanelTitle = styled.span`
  font-size: ${size.h3}px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.8px;
  color: ${color.textDark};
`;

const Toggle = styled.span`
  font-size: 10px;
  color: ${color.textGray};
`;

const Body = styled.div`
  max-height: 180px;
  overflow-y: auto;
  padding: 0 ${space.px16}px ${space.px10}px;
  border-top: ${border.dashed};
`;

const Waiting = styled.div`
  font-size: ${size.bodySm}px;
  color: ${color.textLightGray};
  padding: ${space.px10}px 0;
`;

const Row = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px8}px;
  padding: 3px 0;
  font-size: ${size.bodyXs}px;
`;

const Dot = styled.span<{ $color: string }>`
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: ${(p) => p.$color};
  flex-shrink: 0;
`;

const TypeLabel = styled.span`
  font-weight: 600;
  color: ${color.textDark};
  min-width: 120px;
`;

const Detail = styled.span`
  color: ${color.textGray};
`;
