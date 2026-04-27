import { useState, useCallback } from 'react';
import { Link } from 'react-router-dom';
import styled from 'styled-components';
import { color, font, size, space, border } from '@/theme';
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
              valueColor={color.tealAccent}
            />
            <KvRow
              label="Low"
              value={fmtPrice(kline.low)}
              valueColor={color.redAccent}
            />
            <KvRow label="Close" value={fmtPrice(kline.close)} />
            <KvRow
              label="Source Provider"
              value={kline.source_provider}
              badge
              badgeBg={color.tealAccent}
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
          <Badge style={badgeBg ? { background: badgeBg, color: color.textDark } : undefined}>
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

const TabRow = styled.div`
  display: flex;
  gap: 0;
  margin-bottom: ${space.px10}px;
`;

const TabBtn = styled.button<{ $active: boolean }>`
  all: unset;
  cursor: pointer;
  font-family: ${font.mono};
  font-size: ${size.caption}px;
  font-weight: 700;
  padding: ${space.px4}px ${space.px8}px;
  border: ${border.std};
  border-right: none;
  background: ${(p) => (p.$active ? color.yellowPrimary : color.bgWhite)};
  color: ${color.textDark};

  &:last-child {
    border-right: ${border.std};
  }

  &:hover {
    background: ${(p) => (p.$active ? color.yellowPrimary : color.bgLightGray)};
  }
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

const Badge = styled.span`
  display: inline-block;
  font-size: ${size.mini}px;
  font-weight: 700;
  padding: 1px ${space.px6}px;
  border: ${border.thin};
  background: ${color.bgLightGray};
  color: ${color.textDark};
`;

const ComingSoon = styled.div`
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: ${color.textLightGray};
  font-size: ${size.bodySm}px;
`;

const BottomRow = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: auto;
  padding-top: ${space.px10}px;
  border-top: ${border.dashed};
`;

const CopyBtn = styled.button`
  all: unset;
  cursor: pointer;
  font-family: ${font.mono};
  font-size: ${size.caption}px;
  font-weight: 700;
  padding: ${space.px4}px ${space.px8}px;
  border: ${border.std};
  background: ${color.bgWhite};
  color: ${color.textDark};

  &:hover {
    background: ${color.bgLightGray};
  }
`;

const TraceLink = styled(Link)`
  font-size: ${size.caption}px;
  color: ${color.bluePrimary};
  text-decoration: none;
  font-weight: 700;

  &:hover {
    text-decoration: underline;
  }
`;
