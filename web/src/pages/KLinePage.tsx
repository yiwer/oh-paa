import { useState, useMemo, useCallback } from 'react';
import { useSearchParams } from 'react-router-dom';
import styled from 'styled-components';
import { color, font, size, space, border } from '@/theme';
import { useInstruments } from '@/api/hooks/usePipeline';
import { useCanonicalKlines } from '@/api/hooks/useKline';
import InstrumentSwitcher from '@/components/InstrumentSwitcher/InstrumentSwitcher';
import KLineChart from '@/charts/KLineChart';
import PaBarReading from '@/pages/kline/PaBarReading';
import DataInspector from '@/pages/kline/DataInspector';
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

const CHART_HEIGHT = 500;

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
            <InstrumentSwitcher
              instruments={instruments}
              selectedId={instrumentId}
              onSelect={handleInstrumentSelect}
            />
          )}
        </TopBarLeft>
        <TopBarRight>
          <CheckboxLabel>
            <input
              type="checkbox"
              checked={showPaOverlay}
              onChange={(e) => setShowPaOverlay(e.target.checked)}
            />
            PA Overlay
          </CheckboxLabel>
          <CheckboxLabel>
            <input
              type="checkbox"
              checked={showKeyLevels}
              onChange={(e) => setShowKeyLevels(e.target.checked)}
            />
            Key Levels
          </CheckboxLabel>
          <TfRow>
            {TF_OPTIONS.map((opt) => (
              <TfBtn
                key={opt.value}
                $active={timeframe === opt.value}
                onClick={() => handleTimeframeChange(opt.value)}
              >
                {opt.label}
              </TfBtn>
            ))}
          </TfRow>
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
        <PanelLeft>
          <PaBarReading barReading={selectedBarReading} />
        </PanelLeft>
        <PanelRight>
          <DataInspector kline={selectedKline} />
        </PanelRight>
      </BottomPanels>
    </Root>
  );
}

/* ---- styled ---- */

const Root = styled.div`
  display: flex;
  flex-direction: column;
  gap: ${space.px10}px;
`;

const TopBar = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: ${space.px10}px;
`;

const TopBarLeft = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px8}px;
`;

const TopBarRight = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px12}px;
`;

const CheckboxLabel = styled.label`
  display: flex;
  align-items: center;
  gap: ${space.px4}px;
  font-family: ${font.mono};
  font-size: ${size.caption}px;
  color: ${color.textGray};
  cursor: pointer;
  user-select: none;
`;

const TfRow = styled.div`
  display: flex;
`;

const TfBtn = styled.button<{ $active: boolean }>`
  all: unset;
  cursor: pointer;
  font-family: ${font.mono};
  font-size: 11px;
  font-weight: 700;
  padding: ${space.px4}px ${space.px8}px;
  border: ${border.std};
  border-right: none;
  background: ${(p) => (p.$active ? color.textDark : color.bgWhite)};
  color: ${(p) => (p.$active ? color.yellowPrimary : color.textDark)};
  transition: background-color 0.15s, color 0.15s;

  &:last-child {
    border-right: ${border.std};
  }

  &:hover {
    background: ${(p) => (p.$active ? color.textDark : color.bgLightGray)};
  }
`;

const ChartArea = styled.div`
  border: ${border.std};
  background: ${color.bgWhite};
  min-height: ${CHART_HEIGHT}px;
`;

const ChartEmpty = styled.div`
  display: flex;
  align-items: center;
  justify-content: center;
  height: ${CHART_HEIGHT}px;
  color: ${color.textLightGray};
  font-family: ${font.mono};
  font-size: ${size.bodySm}px;
`;

const BottomPanels = styled.div`
  display: grid;
  grid-template-columns: 320px 1fr;
  gap: ${space.px10}px;

  @media (max-width: 768px) {
    grid-template-columns: 1fr;
  }
`;

const PanelLeft = styled.div`
  min-width: 0;
`;

const PanelRight = styled.div`
  min-width: 0;
`;
