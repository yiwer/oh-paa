# Debug Client: K-Line Charts View — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the K-Line Charts view with ECharts candlestick, crosshair interaction, zoom/pan, PA overlay, hover tooltip, and bottom inspection panels.

**Architecture:** ECharts candlestick chart wrapped in a React component. Data fetched via REST (React Query), open bar updated via WebSocket. Bottom panels show PA bar reading and structured data inspector on bar click/hover.

**Tech Stack:** React 19 · TypeScript · ECharts 5 (echarts-for-react) · styled-components 6 · TanStack React Query 5

**Sub-project scope:** Plan 2 of 3. Builds on Plan 1's foundation (scaffold, design system, AppShell, WS client).

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `web/src/api/hooks/useKline.ts` | React Query hooks for kline and analysis data |
| Create | `web/src/components/InstrumentSwitcher/InstrumentSwitcher.tsx` | Reusable market-grouped pill switcher |
| Create | `web/src/charts/KLineChart.tsx` | ECharts candlestick with crosshair, zoom, tooltip, overlays |
| Create | `web/src/pages/KLinePage.tsx` | Page composition: top bar + chart + bottom panels |
| Create | `web/src/pages/kline/PaBarReading.tsx` | PA bar reading detail panel |
| Create | `web/src/pages/kline/DataInspector.tsx` | Structured key-value data inspector |
| Modify | `web/src/App.tsx` | Replace K-Line placeholder with KLinePage |
| Modify | `web/src/api/types.ts` | Add bar reading and analysis output types |

---

## Task 1: API Hooks + Extended Types for K-Line Data

**Files:**
- Create: `web/src/api/hooks/useKline.ts`
- Modify: `web/src/api/types.ts`

- [ ] **Step 1: Add types for bar reading and analysis output**

In `web/src/api/types.ts`, add:

```typescript
export interface BarReading {
  instrument_id: string;
  timeframe: string;
  bar_close_time: string;
  bar_reading_label: string;
  bar_reading_color: 'red' | 'green' | 'gray' | 'yellow';
  bar_summary: string;
  pattern: string;
  structure: string;
  bias: string;
  source: string;
}

export interface KeyLevel {
  price: string;
  label: string;
  type: 'support' | 'resistance' | 'target';
}

export interface OpenBar {
  instrument_id: string;
  timeframe: string;
  open_time: string;
  open: string;
  high: string;
  low: string;
  close: string;
}
```

- [ ] **Step 2: Create useKline hooks**

```typescript
// web/src/api/hooks/useKline.ts
import { useQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import type { CanonicalKline, OpenBar } from '@/api/types';

export function useCanonicalKlines(
  instrumentId: string,
  timeframe: string,
  limit: number = 48,
) {
  return useQuery({
    queryKey: ['canonical-klines', instrumentId, timeframe, limit],
    queryFn: () =>
      api<{ rows: CanonicalKline[] }>(
        `/market/canonical?instrument_id=${instrumentId}&timeframe=${timeframe}&limit=${limit}&descending=true`,
      ).then((r) => r.rows.reverse()),
    enabled: !!instrumentId,
    staleTime: 15_000,
  });
}

export function useOpenBar(instrumentId: string, timeframe: string) {
  return useQuery({
    queryKey: ['open-bar', instrumentId, timeframe],
    queryFn: () => api<OpenBar>(`/market/open-bar?instrument_id=${instrumentId}&timeframe=${timeframe}`),
    enabled: !!instrumentId,
    staleTime: 5_000,
    refetchInterval: 15_000,
  });
}
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd web && npm run lint`

- [ ] **Step 4: Commit**

`feat(web): add K-Line API hooks and extended types`

---

## Task 2: InstrumentSwitcher Component

**Files:**
- Create: `web/src/components/InstrumentSwitcher/InstrumentSwitcher.tsx`

- [ ] **Step 1: Create reusable instrument switcher**

A row of pill buttons grouped by market, with a dropdown for the full instrument selector.

```typescript
interface Props {
  instruments: Instrument[];
  selectedId: string;
  onSelect: (id: string) => void;
}
```

Features:
- Pill buttons: each instrument's symbol short name (e.g., "BTC", "ETH")
- Active pill: dark bg + yellow text
- Market separator: gray `|` between crypto and forex pills
- Clicking a pill calls `onSelect(instrument.id)`
- Style: 2px solid border, 4px 8px padding, 11px font, JetBrains Mono

- [ ] **Step 2: Verify TypeScript**

Run: `cd web && npm run lint`

- [ ] **Step 3: Commit**

`feat(web): add InstrumentSwitcher component`

---

## Task 3: KLineChart Core Component

**Files:**
- Create: `web/src/charts/KLineChart.tsx`

This is the main ECharts candlestick chart. It must:

1. Register required ECharts modules (CandlestickChart, GridComponent, TooltipComponent, AxisPointerComponent, DataZoomComponent, MarkLineComponent, CanvasRenderer)
2. Accept props: `klines: CanonicalKline[]`, `openBar?: OpenBar`, `selectedBarIndex?: number`, `onBarClick?: (index: number) => void`, `barReadings?: BarReading[]`, `keyLevels?: KeyLevel[]`, `showPaOverlay: boolean`, `showKeyLevels: boolean`
3. Configure:
   - Candlestick series with OHLC data (no volume)
   - Crosshair axis pointer (dashed line, dark bg labels with yellow text on both axes)
   - DataZoom: inside (scroll zoom + drag pan) + slider at bottom
   - Double-click to reset zoom
   - Open bar rendered as last candle with dashed itemStyle
4. Custom HTML tooltip (180px wide):
   - Dark header: bar time + timeframe/state
   - PA info (if available): colored square + pattern name + brief description
   - OHLC: Open/Close/High/Low full names, single column
   - Change: absolute + percentage, colored green/red
5. PA overlay: markPoint on each bar with colored squares (series scatter below candles)
6. Key levels: markLine horizontal dashed lines (support=teal, resistance=red, target=yellow)
7. Click handler: highlight selected bar with yellow itemStyle

- [ ] **Step 1: Create chart component with ECharts registration and basic candlestick**

```typescript
import { useCallback, useMemo, useRef } from 'react';
import ReactEChartsCore from 'echarts-for-react/lib/core';
import * as echarts from 'echarts/core';
import { CandlestickChart } from 'echarts/charts';
import {
  GridComponent, TooltipComponent, AxisPointerComponent,
  DataZoomComponent, MarkLineComponent, MarkPointComponent,
} from 'echarts/components';
import { CanvasRenderer } from 'echarts/renderers';
import type { CanonicalKline, BarReading, KeyLevel, OpenBar } from '@/api/types';
import { color } from '@/theme';

echarts.use([
  CandlestickChart, GridComponent, TooltipComponent,
  AxisPointerComponent, DataZoomComponent, MarkLineComponent,
  MarkPointComponent, CanvasRenderer,
]);
```

Build the full ECharts option object with:
- xAxis: category (formatted bar times)
- yAxis: value (price scale, right side)
- series: candlestick with OHLC
- tooltip: custom HTML formatter
- dataZoom: inside + slider
- axisPointer: crosshair with custom label styles

The component should be ~200-300 lines total. Use `useMemo` for the option object.

- [ ] **Step 2: Add tooltip formatter**

Custom HTML tooltip (180px):
```
┌─────────────────────┐
│ 04-27 13:00  H1·Cls │  ← dark header
│ ■ Bullish Engulfing  │  ← PA (if available, colored)
│   S1 看涨吞没·HL     │  ← PA description
├─────────────────────┤
│ Open    67,432.50    │
│ Close   67,756.80    │
│ High    67,891.00    │  ← teal
│ Low     67,102.30    │  ← red
├─────────────────────┤
│ Change  +324.30      │
│         (+0.48%)     │  ← teal/red
└─────────────────────┘
```

Use `tooltip.formatter` as a function returning HTML string. Escape all values.

- [ ] **Step 3: Add overlays (PA markers + key levels)**

PA overlay: `markPoint` data array with colored squares at each bar position.
Key levels: `markLine` data array with horizontal dashed lines.

- [ ] **Step 4: Add click handler and selected bar highlight**

Use `onEvents` prop on ReactEChartsCore to listen for click events. Call `onBarClick(dataIndex)`.

- [ ] **Step 5: Verify TypeScript compiles**

Run: `cd web && npm run lint`

- [ ] **Step 6: Commit**

`feat(web): add KLineChart ECharts component with crosshair, zoom, tooltip, and overlays`

---

## Task 4: Bottom Panels (PA Bar Reading + Data Inspector)

**Files:**
- Create: `web/src/pages/kline/PaBarReading.tsx`
- Create: `web/src/pages/kline/DataInspector.tsx`

- [ ] **Step 1: Create PaBarReading panel**

Props: `{ barReading?: BarReading }`

When no bar selected: show "click a bar to inspect" placeholder.
When selected: colored square + pattern name (bold) + bar time + timeframe, analysis summary text, bottom tags (Structure / Bias / Source).

- [ ] **Step 2: Create DataInspector panel**

Props: `{ kline?: CanonicalKline }`

Tab row: Canonical | Raw | Aggregated | PA State (only Canonical implemented in MVP).
Structured key-value table (NOT raw JSON):
- Instrument, Timeframe, Open Time, Close Time
- Open, High (teal), Low (red), Close
- Source Provider (badge), Bar State (badge)
Bottom: Copy button + "Open in LLM Trace →" link.

- [ ] **Step 3: Verify TypeScript**

Run: `cd web && npm run lint`

- [ ] **Step 4: Commit**

`feat(web): add PA Bar Reading and Data Inspector panels`

---

## Task 5: KLinePage Composition + Router

**Files:**
- Create: `web/src/pages/KLinePage.tsx`
- Modify: `web/src/App.tsx`

- [ ] **Step 1: Create KLinePage**

Layout:
1. Top bar: InstrumentSwitcher (left) + zoom controls (center-right) + timeframe tabs M15|H1|D1 (right)
2. KLineChart (main area)
3. Bottom: two-column flex — PaBarReading (left) + DataInspector (right)

State:
- `selectedInstrumentId` (from URL param or first instrument)
- `selectedTimeframe` ('15m' | '1h' | '1d', default '1h')
- `selectedBarIndex` (null until clicked)

Data flow:
- `useInstruments()` for instrument list
- `useCanonicalKlines(id, tf)` for chart data
- `useOpenBar(id, tf)` for live bar
- WS `open_bar_update` events update the open bar via store

- [ ] **Step 2: Wire into router**

In App.tsx, replace kline placeholder:
```tsx
import KLinePage from '@/pages/KLinePage';
<Route path="/kline" element={<KLinePage />} />
```

- [ ] **Step 3: Verify TypeScript**

Run: `cd web && npm run lint`

- [ ] **Step 4: Commit**

`feat(web): implement K-Line Charts page with full composition`

---

## Self-Review

- [x] Crosshair with price/time labels on axes
- [x] Scroll zoom, drag pan, double-click reset, dataZoom slider
- [x] Zoom limits per timeframe (configured in chart options)
- [x] Hover tooltip: OHLC full names + Change (abs + %) + PA info in header
- [x] PA overlay toggle, Key Levels toggle
- [x] Open Bar with live WS update
- [x] Click bar → bottom panels update
- [x] PA Bar Reading panel
- [x] Data Inspector (structured, no JSON)
- [x] Instrument switcher (reusable)
- [x] Timeframe tabs
