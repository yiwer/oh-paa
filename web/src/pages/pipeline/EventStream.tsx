import { useState, useRef, useEffect } from 'react';
import styled from 'styled-components';
import { color, font, border, radius, shadow, space } from '@/theme';
import type { DebugEvent, DebugEventType } from '@/api/types';

interface Props {
  events: DebugEvent[];
}

const dotColors: Record<DebugEventType, string> = {
  kline_ingested: color.teal,
  provider_fallback: color.yellow,
  normalization_result: color.red,
  task_status_changed: color.blue,
  attempt_completed: color.blue,
  open_bar_update: color.teal,
};

function dotColor(type: string): string {
  return dotColors[type as DebugEventType] ?? color.text3;
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
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: ${radius.card};
  box-shadow: ${shadow.card};
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
  font-family: ${font.ui};
  font-size: 14px;
  font-weight: 700;
  color: ${color.text1};
`;

const Toggle = styled.span`
  font-size: 10px;
  color: ${color.text3};
`;

const Body = styled.div`
  max-height: 220px;
  overflow-y: auto;
  padding: ${space.px8}px ${space.px16}px ${space.px10}px;
  border-top: ${border.dashed};
`;

const Waiting = styled.div`
  font-family: ${font.ui};
  font-size: 13px;
  color: ${color.text3};
  padding: ${space.px10}px 0;
`;

const Row = styled.div`
  display: flex;
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

const Dot = styled.span<{ $color: string }>`
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: ${(p) => p.$color};
  flex-shrink: 0;
`;

const TypeLabel = styled.span`
  font-weight: 600;
  color: ${color.text1};
  min-width: 120px;
`;

const Detail = styled.span`
  color: ${color.text2};
`;
