import { useMemo } from 'react';
import styled from 'styled-components';
import { color, space, size } from '@/theme';
import MetricCard, { MetricStrip } from '@/components/MetricCard/MetricCard';
import { useDebugEventStore } from '@/ws/debugEventStore';
import { useInstruments } from '@/api/hooks/usePipeline';
import InstrumentCard from '@/pages/pipeline/InstrumentCard';
import type { Instrument, DebugEvent } from '@/api/types';

function isCrypto(symbol: string) {
  return symbol.includes('USDT');
}

function groupInstruments(instruments: Instrument[]) {
  const crypto: Instrument[] = [];
  const forex: Instrument[] = [];
  for (const inst of instruments) {
    if (isCrypto(inst.symbol)) {
      crypto.push(inst);
    } else {
      forex.push(inst);
    }
  }
  return { crypto, forex };
}

function eventsForInstrument(events: DebugEvent[], instrumentId: string) {
  return events.filter((e) => e.instrument_id === instrumentId);
}

function hasErrorEvents(events: DebugEvent[]) {
  return events.some(
    (e) => e.type === 'provider_fallback' || e.type === 'normalization_result',
  );
}

export default function PipelinePage() {
  const events = useDebugEventStore((s) => s.events);
  const { data: instruments = [] } = useInstruments();

  const klineCount = useMemo(
    () => events.filter((e) => e.type === 'kline_ingested').length,
    [events],
  );

  const errorCount = useMemo(
    () =>
      events.filter(
        (e) =>
          e.type === 'provider_fallback' || e.type === 'normalization_result',
      ).length,
    [events],
  );

  const { crypto, forex } = useMemo(
    () => groupInstruments(instruments),
    [instruments],
  );

  return (
    <Root>
      <Title>Market Data Pipeline</Title>
      <Subtitle>{'实时数据摄入 & Provider 路由状态'}</Subtitle>

      <MetricStrip style={{ marginBottom: space.px24 }}>
        <MetricCard
          accent="teal"
          eyebrow="Klines Ingested"
          value={klineCount}
        />
        <MetricCard
          accent="blue"
          eyebrow="Provider Routes"
          value={instruments.length}
        />
        <MetricCard
          accent="yellow"
          eyebrow="Normalization"
          value={'\u2014'}
        />
        <MetricCard accent="red" eyebrow="Errors" value={errorCount} />
      </MetricStrip>

      {crypto.length > 0 && (
        <section>
          <GroupTitle>Crypto</GroupTitle>
          <CardGrid>
            {crypto.map((inst) => {
              const instEvents = eventsForInstrument(events, inst.id);
              return (
                <InstrumentCard
                  key={inst.id}
                  instrument={inst}
                  events={instEvents}
                  hasError={hasErrorEvents(instEvents)}
                />
              );
            })}
          </CardGrid>
        </section>
      )}

      {forex.length > 0 && (
        <section>
          <GroupTitle>Forex</GroupTitle>
          <CardGrid>
            {forex.map((inst) => {
              const instEvents = eventsForInstrument(events, inst.id);
              return (
                <InstrumentCard
                  key={inst.id}
                  instrument={inst}
                  events={instEvents}
                  hasError={hasErrorEvents(instEvents)}
                />
              );
            })}
          </CardGrid>
        </section>
      )}

    </Root>
  );
}

/* ---- styled ---- */

const Root = styled.div`
  display: flex;
  flex-direction: column;
`;

const Title = styled.h2`
  font-size: ${size.h2}px;
  font-weight: 800;
  color: ${color.textDark};
  margin: 0 0 ${space.px4}px 0;
`;

const Subtitle = styled.p`
  font-size: ${size.bodySm}px;
  color: ${color.textGray};
  margin: 0 0 ${space.px20}px 0;
`;

const GroupTitle = styled.h3`
  font-size: ${size.eyebrow}px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 1px;
  color: ${color.textGray};
  margin: 0 0 ${space.px8}px 0;
`;

const CardGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: ${space.px10}px;
  margin-bottom: ${space.px24}px;
`;
