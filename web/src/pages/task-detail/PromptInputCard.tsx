import { useState } from 'react';
import styled from 'styled-components';
import { color, font, size, space, border } from '@/theme';
import type { AnalysisTask } from '@/api/types';

interface Props {
  task: AnalysisTask;
  snapshot?: Record<string, unknown>;
}

interface KlineRow {
  time: string;
  open: string;
  high: string;
  low: string;
  close: string;
}

function extractKlines(snapshot: Record<string, unknown>): KlineRow[] {
  const kline = snapshot.kline ?? snapshot.klines ?? snapshot.bars ?? snapshot.k_line;
  if (!Array.isArray(kline)) return [];
  return kline.map((k: Record<string, unknown>) => ({
    time: String(k.close_time ?? k.open_time ?? '--'),
    open: String(k.open ?? '--'),
    high: String(k.high ?? '--'),
    low: String(k.low ?? '--'),
    close: String(k.close ?? '--'),
  }));
}

function extractPaState(
  snapshot: Record<string, unknown>,
): Record<string, string> | null {
  const pa =
    (snapshot.pa_state as Record<string, unknown> | undefined) ??
    (snapshot.upstream_pa_state as Record<string, unknown> | undefined);
  if (!pa) return null;
  return {
    Structure: String(pa.structure ?? '--'),
    Bias: String(pa.bias ?? '--'),
    Resistance: String(pa.resistance ?? '--'),
    Support: String(pa.support ?? '--'),
    Source: String(pa.source ?? '--'),
  };
}

function fmtTime(iso: string): string {
  if (iso === '--') return iso;
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  const hh = String(d.getHours()).padStart(2, '0');
  const mi = String(d.getMinutes()).padStart(2, '0');
  return `${mm}-${dd} ${hh}:${mi}`;
}

const INITIAL_KLINE_ROWS = 4;

export default function PromptInputCard({ task, snapshot }: Props) {
  const [expanded, setExpanded] = useState(false);

  if (!snapshot) {
    return (
      <Root>
        <Title>Prompt Input</Title>
        <Placeholder>Snapshot data not loaded</Placeholder>
      </Root>
    );
  }

  const klines = extractKlines(snapshot);
  const paState = extractPaState(snapshot);
  const targetBarTime = task.bar_close_time;
  const hiddenCount =
    klines.length > INITIAL_KLINE_ROWS && !expanded
      ? klines.length - INITIAL_KLINE_ROWS
      : 0;
  const visibleKlines = expanded ? klines : klines.slice(0, INITIAL_KLINE_ROWS);

  return (
    <Root>
      <Title>Prompt Input</Title>

      <Columns>
        {/* Task Context */}
        <Column>
          <SectionLabel>Task Context</SectionLabel>
          <KvTable>
            <tbody>
              <KvRow label="Instrument ID" value={task.instrument_id.slice(0, 12)} />
              <KvRow label="Timeframe" value={task.timeframe?.toUpperCase() ?? '--'} />
              <KvRow label="Bar Window" value={`${klines.length} bars`} />
              <KvRow label="Bar State" value={task.bar_state} />
              <KvRow label="Input Hash" value={task.snapshot_id.slice(0, 12)} />
              <KvRow label="Dedupe Key" value={task.prompt_key} />
            </tbody>
          </KvTable>
        </Column>

        {/* K-Line Input */}
        {klines.length > 0 && (
          <Column>
            <SectionLabel>K-Line Input</SectionLabel>
            <DataTable>
              <thead>
                <tr>
                  <Th>Time</Th>
                  <Th>Open</Th>
                  <Th>High</Th>
                  <Th>Low</Th>
                  <Th>Close</Th>
                </tr>
              </thead>
              <tbody>
                {visibleKlines.map((k, i) => {
                  const isTarget =
                    targetBarTime != null && k.time === targetBarTime;
                  return (
                    <DataTr key={i} $highlight={isTarget}>
                      <DataTd>{fmtTime(k.time)}</DataTd>
                      <DataTd>{k.open}</DataTd>
                      <DataTd>{k.high}</DataTd>
                      <DataTd>{k.low}</DataTd>
                      <DataTd>{k.close}</DataTd>
                    </DataTr>
                  );
                })}
              </tbody>
            </DataTable>
            {hiddenCount > 0 && (
              <ExpandBtn onClick={() => setExpanded(true)}>
                {hiddenCount} more bars...
              </ExpandBtn>
            )}
            {expanded && klines.length > INITIAL_KLINE_ROWS && (
              <ExpandBtn onClick={() => setExpanded(false)}>
                show less
              </ExpandBtn>
            )}
          </Column>
        )}

        {/* PA State */}
        {paState && (
          <Column>
            <SectionLabel>PA State</SectionLabel>
            <KvTable>
              <tbody>
                {Object.entries(paState).map(([k, v]) => (
                  <KvRow key={k} label={k} value={v} />
                ))}
              </tbody>
            </KvTable>
          </Column>
        )}
      </Columns>

      <BottomLink>View full prompt text &rarr;</BottomLink>
    </Root>
  );
}

/* ---- KvRow helper ---- */

function KvRow({ label, value }: { label: string; value: string }) {
  return (
    <Tr>
      <FieldTd>{label}</FieldTd>
      <ValueTd>{value}</ValueTd>
    </Tr>
  );
}

/* ---- styled ---- */

const Root = styled.div`
  background: ${color.bgWhite};
  border: ${border.std};
  padding: ${space.px16}px;
  font-family: ${font.mono};
`;

const Title = styled.h4`
  font-size: ${size.eyebrow}px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 1px;
  color: ${color.textGray};
  margin: 0 0 ${space.px12}px 0;
`;

const Placeholder = styled.span`
  font-size: ${size.bodySm}px;
  color: ${color.textLightGray};
`;

const Columns = styled.div`
  display: flex;
  gap: ${space.px16}px;
  flex-wrap: wrap;
`;

const Column = styled.div`
  flex: 1;
  min-width: 180px;
`;

const SectionLabel = styled.div`
  font-size: ${size.mini}px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: ${color.textLightGray};
  margin-bottom: ${space.px6}px;
`;

const KvTable = styled.table`
  width: 100%;
  border-collapse: collapse;
  font-size: ${size.caption}px;
`;

const Tr = styled.tr`
  border-bottom: ${border.dashed};

  &:last-child {
    border-bottom: none;
  }
`;

const FieldTd = styled.td`
  padding: ${space.px4}px 0;
  color: ${color.textGray};
  font-size: ${size.caption}px;
  white-space: nowrap;
`;

const ValueTd = styled.td`
  padding: ${space.px4}px 0;
  text-align: right;
  color: ${color.textDark};
  font-size: ${size.bodyXs}px;
`;

const DataTable = styled.table`
  width: 100%;
  border-collapse: collapse;
  font-size: ${size.caption}px;
`;

const Th = styled.th`
  text-align: right;
  padding: ${space.px4}px ${space.px4}px;
  color: ${color.textLightGray};
  font-size: ${size.mini}px;
  font-weight: 700;
  border-bottom: ${border.thin};

  &:first-child {
    text-align: left;
  }
`;

const DataTr = styled.tr<{ $highlight: boolean }>`
  background: ${(p) => (p.$highlight ? 'rgba(255, 222, 0, 0.15)' : 'transparent')};
  border-bottom: ${border.dashed};

  &:last-child {
    border-bottom: none;
  }
`;

const DataTd = styled.td`
  padding: ${space.px4}px ${space.px4}px;
  color: ${color.textDark};
  font-size: ${size.caption}px;
  text-align: right;

  &:first-child {
    text-align: left;
    color: ${color.textGray};
  }
`;

const ExpandBtn = styled.button`
  all: unset;
  cursor: pointer;
  font-family: ${font.mono};
  font-size: ${size.caption}px;
  color: ${color.bluePrimary};
  font-weight: 700;
  margin-top: ${space.px4}px;
  display: block;

  &:hover {
    text-decoration: underline;
  }
`;

const BottomLink = styled.div`
  font-size: ${size.caption}px;
  color: ${color.bluePrimary};
  font-weight: 700;
  margin-top: ${space.px12}px;
  padding-top: ${space.px8}px;
  border-top: ${border.dashed};
  cursor: pointer;

  &:hover {
    text-decoration: underline;
  }
`;
