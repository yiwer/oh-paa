import { useState, useMemo, useCallback } from 'react';
import { useSearchParams } from 'react-router-dom';
import styled from 'styled-components';
import { color, font, border, radius, shadow, space } from '@/theme';
import { useInstruments } from '@/api/hooks/usePipeline';
import { useCanonicalKlines } from '@/api/hooks/useKline';
import InstrumentDropdown from '@/components/Dropdown/Dropdown';
import Segmented from '@/components/Segmented/Segmented';
import KLineChart from '@/charts/KLineChart';
import PaBarReading from '@/pages/kline/PaBarReading';
import DataInspector from '@/pages/kline/DataInspector';
import KeyLevels from '@/pages/kline/KeyLevels';
import type { BarReading, KeyLevel } from '@/api/types';

/* ------------------------------------------------------------------ */
/*  Constants                                                          */
/* ------------------------------------------------------------------ */

type Timeframe = '15m' | '1h' | '1d';

const TF_OPTIONS: { value: Timeframe; label: string }[] = [
  { value: '15m', label: 'M15' },
  { value: '1h', label: 'H1' },
  { value: '1d', label: 'D1' },
];

const CHART_HEIGHT = 480;

/* ------------------------------------------------------------------ */
/*  Component                                                          */
/* ------------------------------------------------------------------ */

export default function KLinePage() {
  const { data: instruments = [] } = useInstruments();
  const [searchParams] = useSearchParams();

  /* ---- state (seeded from URL search params) ---- */
  const [selectedInstrumentId, setSelectedInstrumentId] = useState<string>(
    () => searchParams.get('instrument') ?? '',
  );
  const [timeframe, setTimeframe] = useState<Timeframe>(() => {
    const param = searchParams.get('timeframe');
    if (param === '15m' || param === '1h' || param === '1d') return param;
    return '1h';
  });
  const [selectedBarIndex, setSelectedBarIndex] = useState<number | null>(null);
  const [showPaOverlay, setShowPaOverlay] = useState(false);
  const [showKeyLevels, setShowKeyLevels] = useState(false);

  // Auto-select first instrument when loaded
  const instrumentId = selectedInstrumentId || (instruments.length > 0 ? instruments[0].id : '');

  const { data: klines = [] } = useCanonicalKlines(instrumentId, timeframe);

  /* ---- derived selections ---- */
  const selectedKline = useMemo(
    () => (selectedBarIndex != null ? klines[selectedBarIndex] ?? undefined : undefined),
    [klines, selectedBarIndex],
  );

  // Bar readings and key levels: empty arrays for now
  const barReadings: BarReading[] = useMemo(() => [], []);
  const keyLevels: KeyLevel[] = useMemo(() => [], []);

  const selectedBarReading: BarReading | undefined = useMemo(() => {
    if (selectedBarIndex == null || !selectedKline) return undefined;
    return barReadings.find((r) => r.bar_close_time === selectedKline.close_time);
  }, [selectedBarIndex, selectedKline, barReadings]);

  /* ---- callbacks ---- */
  const handleBarClick = useCallback((index: number) => {
    setSelectedBarIndex(index);
  }, []);

  const handleInstrumentSelect = useCallback((id: string) => {
    setSelectedInstrumentId(id);
    setSelectedBarIndex(null);
  }, []);

  const handleTimeframeChange = useCallback((tf: Timeframe) => {
    setTimeframe(tf);
    setSelectedBarIndex(null);
  }, []);

  /* ---- render ---- */
  return (
    <Root>
      {/* Top Bar */}
      <TopBar>
        <TopBarLeft>
          {instruments.length > 0 && (
            <InstrumentDropdown
              instruments={instruments}
              selectedId={instrumentId}
              onSelect={handleInstrumentSelect}
              label="Instrument"
            />
          )}
        </TopBarLeft>
        <TopBarRight>
          <CheckboxPill $on={showPaOverlay}>
            <input
              type="checkbox"
              checked={showPaOverlay}
              onChange={(e) => setShowPaOverlay(e.target.checked)}
            />
            PA Overlay
          </CheckboxPill>
          <CheckboxPill $on={showKeyLevels}>
            <input
              type="checkbox"
              checked={showKeyLevels}
              onChange={(e) => setShowKeyLevels(e.target.checked)}
            />
            Key Levels
          </CheckboxPill>
          <Segmented<Timeframe>
            options={TF_OPTIONS}
            value={timeframe}
            onChange={handleTimeframeChange}
            variant="mono"
          />
        </TopBarRight>
      </TopBar>

      {/* Chart */}
      <ChartArea>
        {klines.length === 0 ? (
          <ChartEmpty>No kline data for this instrument / timeframe</ChartEmpty>
        ) : (
          <KLineChart
            klines={klines}
            barReadings={barReadings}
            keyLevels={keyLevels}
            showPaOverlay={showPaOverlay}
            showKeyLevels={showKeyLevels}
            selectedBarIndex={selectedBarIndex}
            onBarClick={handleBarClick}
            timeframe={timeframe}
            height={CHART_HEIGHT}
          />
        )}
      </ChartArea>

      {/* Bottom Panels */}
      <BottomPanels>
        <PaBarReading barReading={selectedBarReading} />
        <DataInspector kline={selectedKline} />
        <KeyLevels kline={selectedKline} keyLevels={keyLevels} />
      </BottomPanels>
    </Root>
  );
}

/* ---- styled ---- */

const Root = styled.div`
  display: flex;
  flex-direction: column;
  gap: ${space.px12}px;
`;

const TopBar = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: ${space.px12}px;
`;

const TopBarLeft = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px8}px;
`;

const TopBarRight = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px8}px;
`;

const CheckboxPill = styled.label<{ $on: boolean }>`
  display: inline-flex;
  align-items: center;
  gap: ${space.px6}px;
  padding: 5px ${space.px10}px;
  font-family: ${font.ui};
  font-size: 12px;
  color: ${(p) => (p.$on ? color.text1 : color.text2)};
  background: ${(p) => (p.$on ? color.bgSurface : 'transparent')};
  border: ${(p) => (p.$on ? border.default : '1px solid transparent')};
  border-radius: ${radius.control};
  cursor: pointer;
  user-select: none;
`;

const ChartArea = styled.div`
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: ${radius.card};
  box-shadow: ${shadow.card};
  min-height: ${CHART_HEIGHT}px;
  padding: ${space.px8}px;
`;

const ChartEmpty = styled.div`
  display: flex;
  align-items: center;
  justify-content: center;
  height: ${CHART_HEIGHT}px;
  color: ${color.text3};
  font-family: ${font.ui};
  font-size: 13px;
`;

const BottomPanels = styled.div`
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: ${space.px12}px;

  @media (max-width: 900px) {
    grid-template-columns: 1fr;
  }
`;
