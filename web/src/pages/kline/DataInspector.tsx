import { useState, useCallback } from 'react';
import { Link } from 'react-router-dom';
import styled from 'styled-components';
import { color, font, border, radius, shadow, space } from '@/theme';
import type { CanonicalKline } from '@/api/types';

interface Props {
  kline?: CanonicalKline;
}

type Tab = 'canonical' | 'raw' | 'aggregated' | 'pa_state';

const TABS: { key: Tab; label: string }[] = [
  { key: 'canonical', label: 'Canonical' },
  { key: 'raw', label: 'Raw' },
  { key: 'aggregated', label: 'Aggregated' },
  { key: 'pa_state', label: 'PA State' },
];

function fmtDatetime(iso: string): string {
  const d = new Date(iso);
  const yyyy = d.getFullYear();
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  const hh = String(d.getHours()).padStart(2, '0');
  const mi = String(d.getMinutes()).padStart(2, '0');
  const ss = String(d.getSeconds()).padStart(2, '0');
  return `${yyyy}-${mm}-${dd} ${hh}:${mi}:${ss}`;
}

function fmtPrice(v: string): string {
  const n = Number(v);
  if (n >= 1000) return n.toFixed(2);
  if (n >= 1) return n.toFixed(4);
  return n.toFixed(6);
}

export default function DataInspector({ kline }: Props) {
  const [activeTab, setActiveTab] = useState<Tab>('canonical');

  const handleCopy = useCallback(() => {
    if (!kline) return;
    navigator.clipboard.writeText(JSON.stringify(kline, null, 2)).catch(() => {
      /* noop */
    });
  }, [kline]);

  if (!kline) {
    return (
      <Root>
        <Title>Data Inspector</Title>
        <EmptyHint>select a bar to inspect</EmptyHint>
      </Root>
    );
  }

  return (
    <Root>
      <Title>Data Inspector</Title>

      <TabRow>
        {TABS.map((t) => (
          <TabBtn
            key={t.key}
            $active={activeTab === t.key}
            onClick={() => setActiveTab(t.key)}
          >
            {t.label}
          </TabBtn>
        ))}
      </TabRow>

      {activeTab === 'canonical' ? (
        <KvTable>
          <tbody>
            <KvRow label="Instrument" value={kline.instrument_id.slice(0, 8)} />
            <KvRow label="Timeframe" value={kline.timeframe.toUpperCase()} />
            <KvRow label="Open Time" value={fmtDatetime(kline.open_time)} />
            <KvRow label="Close Time" value={fmtDatetime(kline.close_time)} />
            <KvRow label="Open" value={fmtPrice(kline.open)} />
            <KvRow
              label="High"
              value={fmtPrice(kline.high)}
              valueColor={color.tealText}
            />
            <KvRow
              label="Low"
              value={fmtPrice(kline.low)}
              valueColor={color.redText}
            />
            <KvRow label="Close" value={fmtPrice(kline.close)} />
            <KvRow
              label="Source Provider"
              value={kline.source_provider}
              badge
              badgeBg={color.tealSoft}
            />
            <KvRow label="Bar State" value="Closed" badge />
          </tbody>
        </KvTable>
      ) : (
        <ComingSoon>Coming soon</ComingSoon>
      )}

      <BottomRow>
        <CopyBtn onClick={handleCopy}>Copy</CopyBtn>
        <TraceLink to="/llm-trace">Open in LLM Trace &rarr;</TraceLink>
      </BottomRow>
    </Root>
  );
}

/* ---- KvRow helper ---- */

function KvRow({
  label,
  value,
  valueColor,
  badge,
  badgeBg,
}: {
  label: string;
  value: string;
  valueColor?: string;
  badge?: boolean;
  badgeBg?: string;
}) {
  return (
    <Tr>
      <FieldTd>{label}</FieldTd>
      <ValueTd>
        {badge ? (
          <Badge style={badgeBg ? { background: badgeBg, color: color.tealText } : undefined}>
            {value}
          </Badge>
        ) : (
          <span style={valueColor ? { color: valueColor } : undefined}>
            {value}
          </span>
        )}
      </ValueTd>
    </Tr>
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

const TabRow = styled.div`
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 2px;
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: ${radius.control};
  margin-bottom: ${space.px10}px;
`;

const TabBtn = styled.button<{ $active: boolean }>`
  all: unset;
  cursor: pointer;
  font-family: ${font.ui};
  font-size: 11px;
  font-weight: 600;
  padding: 4px ${space.px8}px;
  border-radius: 4px;
  background: ${(p) => (p.$active ? color.yellow : 'transparent')};
  color: ${(p) => (p.$active ? color.text1 : color.text2)};

  &:hover {
    color: ${color.text1};
    background: ${(p) => (p.$active ? color.yellow : color.bgSoft)};
  }
`;

const KvTable = styled.table`
  width: 100%;
  border-collapse: collapse;
  font-family: ${font.mono};
  font-size: 11px;
`;

const Tr = styled.tr`
  border-bottom: ${border.dashed};

  &:last-child {
    border-bottom: none;
  }
`;

const FieldTd = styled.td`
  padding: 4px 0;
  color: ${color.text2};
  font-family: ${font.ui};
  font-size: 11px;
  white-space: nowrap;
`;

const ValueTd = styled.td`
  padding: 4px 0;
  text-align: right;
  color: ${color.text1};
  font-size: 12px;
`;

const Badge = styled.span`
  display: inline-block;
  font-family: ${font.ui};
  font-size: 10px;
  font-weight: 600;
  padding: 2px ${space.px8}px;
  border-radius: ${radius.tag};
  background: ${color.bgSoft};
  color: ${color.text2};
`;

const ComingSoon = styled.div`
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: ${color.text3};
  font-size: 13px;
`;

const BottomRow = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: auto;
  padding-top: ${space.px8}px;
  border-top: ${border.dashed};
`;

const CopyBtn = styled.button`
  all: unset;
  cursor: pointer;
  font-family: ${font.ui};
  font-size: 11px;
  font-weight: 600;
  padding: 4px ${space.px10}px;
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: ${radius.control};
  color: ${color.text1};

  &:hover {
    background: ${color.bgSoft};
  }
`;

const TraceLink = styled(Link)`
  font-family: ${font.ui};
  font-size: 11px;
  color: ${color.blueText};
  text-decoration: none;
  font-weight: 600;

  &:hover {
    text-decoration: underline;
  }
`;
