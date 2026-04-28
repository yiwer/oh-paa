import { useMemo, useCallback, useRef } from 'react';
import ReactEChartsCore from 'echarts-for-react/lib/core';
import * as echarts from 'echarts/core';
import { CandlestickChart, ScatterChart } from 'echarts/charts';
import {
  GridComponent,
  TooltipComponent,
  AxisPointerComponent,
  DataZoomComponent,
  MarkLineComponent,
} from 'echarts/components';
import { CanvasRenderer } from 'echarts/renderers';
import type { EChartsOption } from 'echarts';

import type {
  CanonicalKline,
  OpenBar,
  BarReading,
  KeyLevel,
} from '@/api/types';
import { color, font } from '@/theme';

echarts.use([
  CandlestickChart,
  ScatterChart,
  GridComponent,
  TooltipComponent,
  AxisPointerComponent,
  DataZoomComponent,
  MarkLineComponent,
  CanvasRenderer,
]);

/* ------------------------------------------------------------------ */
/*  Props                                                              */
/* ------------------------------------------------------------------ */

interface KLineChartProps {
  klines: CanonicalKline[];
  openBar?: OpenBar;
  barReadings?: BarReading[];
  keyLevels?: KeyLevel[];
  showPaOverlay: boolean;
  showKeyLevels: boolean;
  selectedBarIndex: number | null;
  onBarClick: (index: number) => void;
  timeframe: string;
  height?: number;
}

/* ------------------------------------------------------------------ */
/*  Helpers                                                            */
/* ------------------------------------------------------------------ */

const PA_COLOR_MAP: Record<string, string> = {
  green: color.teal,
  red: color.red,
  gray: color.text2,
  yellow: color.yellow,
};

const KEY_LEVEL_COLOR: Record<string, string> = {
  support: color.teal,
  resistance: color.red,
  target: color.yellow,
};

/** Format ISO datetime to compact axis label. */
function fmtAxisTime(iso: string): string {
  const d = new Date(iso);
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  const hh = String(d.getHours()).padStart(2, '0');
  const mi = String(d.getMinutes()).padStart(2, '0');
  return `${mm}-${dd} ${hh}:${mi}`;
}

/** Format price to a reasonable decimal string. */
function fmtPrice(v: number): string {
  if (v >= 1000) return v.toFixed(2);
  if (v >= 1) return v.toFixed(4);
  return v.toFixed(6);
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/* ------------------------------------------------------------------ */
/*  Component                                                          */
/* ------------------------------------------------------------------ */

export default function KLineChart({
  klines,
  openBar,
  barReadings,
  keyLevels,
  showPaOverlay,
  showKeyLevels,
  selectedBarIndex,
  onBarClick,
  timeframe,
  height = 500,
}: KLineChartProps) {
  const chartRef = useRef<ReactEChartsCore>(null);

  /* ---------- derived data ---------- */

  /** All bars: closed klines + optional open bar appended. */
  const allBars = useMemo(() => {
    const bars = klines.map((k) => ({
      time: k.open_time,
      open: Number(k.open),
      high: Number(k.high),
      low: Number(k.low),
      close: Number(k.close),
      isOpen: false,
    }));

    if (openBar) {
      bars.push({
        time: openBar.open_time,
        open: Number(openBar.open),
        high: Number(openBar.high),
        low: Number(openBar.low),
        close: Number(openBar.close),
        isOpen: true,
      });
    }

    return bars;
  }, [klines, openBar]);

  /** Category axis labels. */
  const categoryData = useMemo(
    () => allBars.map((b) => fmtAxisTime(b.time)),
    [allBars],
  );

  /** Build a lookup: formatted-time -> BarReading. */
  const readingByTime = useMemo(() => {
    const map = new Map<string, BarReading>();
    if (!barReadings) return map;
    for (const r of barReadings) {
      // Match on close_time formatted identically to how we format open_time for klines.
      // We need to match by close_time from reading to the kline that has matching close_time.
      map.set(r.bar_close_time, r);
    }
    return map;
  }, [barReadings]);

  /** Find reading for a bar index. */
  const readingForIndex = useCallback(
    (idx: number): BarReading | undefined => {
      if (!barReadings || idx < 0 || idx >= klines.length) return undefined;
      return readingByTime.get(klines[idx].close_time);
    },
    [barReadings, readingByTime, klines],
  );

  /* ---------- echarts option ---------- */

  const option = useMemo((): EChartsOption => {
    // Per-item style for selected bar and open bar
    // ECharts candlestick data order: [open, close, low, high]
    const itemStyles = allBars.map((b, i) => {
      const isSelected = selectedBarIndex === i;
      const isUp = b.close >= b.open;

      if (b.isOpen) {
        return {
          value: [b.open, b.close, b.low, b.high],
          itemStyle: {
            color: isUp ? 'rgba(47,184,154,0.3)' : 'rgba(229,104,91,0.3)',
            color0: isUp ? 'rgba(47,184,154,0.3)' : 'rgba(229,104,91,0.3)',
            borderColor: isUp ? color.tealText : color.redText,
            borderColor0: isUp ? color.tealText : color.redText,
            borderType: 'dashed' as const,
            borderWidth: 2,
          },
        };
      }

      if (isSelected) {
        return {
          value: [b.open, b.close, b.low, b.high],
          itemStyle: {
            borderColor: color.yellow,
            borderColor0: color.yellow,
            borderWidth: 2,
          },
        };
      }

      return [b.open, b.close, b.low, b.high];
    });

    // PA overlay scatter series
    const paScatterData: (
      | { value: [number, number]; itemStyle: { color: string } }
      | null
    )[] = [];

    if (showPaOverlay && barReadings) {
      for (let i = 0; i < allBars.length; i++) {
        const reading = readingForIndex(i);
        if (reading) {
          const yPos = allBars[i].low - (allBars[i].high - allBars[i].low) * 0.15;
          paScatterData.push({
            value: [i, yPos],
            itemStyle: {
              color: PA_COLOR_MAP[reading.bar_reading_color] ?? color.text2,
            },
          });
        } else {
          paScatterData.push(null);
        }
      }
    }

    // Key level mark lines
    const markLineData: {
      yAxis: number;
      name: string;
      lineStyle: { color: string; type: 'dashed' };
      label: {
        formatter: string;
        position: 'insideEndTop';
        color: string;
        fontSize: number;
      };
    }[] = [];

    if (showKeyLevels && keyLevels) {
      for (const kl of keyLevels) {
        const clr = KEY_LEVEL_COLOR[kl.type] ?? color.text2;
        markLineData.push({
          yAxis: Number(kl.price),
          name: kl.label,
          lineStyle: { color: clr, type: 'dashed' },
          label: {
            formatter: `${kl.label}  ${fmtPrice(Number(kl.price))}`,
            position: 'insideEndTop',
            color: clr,
            fontSize: 10,
          },
        });
      }
    }

    // Tooltip formatter
    const tooltipFormatter = (
      params: { dataIndex: number; data: number[] | { value: number[] } }[],
    ): string => {
      const p = params[0];
      if (!p) return '';
      const idx = p.dataIndex;
      const bar = allBars[idx];
      if (!bar) return '';

      const o = bar.open;
      const c = bar.close;
      const h = bar.high;
      const l = bar.low;
      const change = c - o;
      const changePct = o !== 0 ? (change / o) * 100 : 0;
      const isUp = change >= 0;
      const changeColor = isUp ? color.teal : color.red;
      const state = bar.isOpen ? 'Open' : 'Closed';
      const reading = readingForIndex(idx);

      let paHtml = '';
      if (reading) {
        const dotColor =
          PA_COLOR_MAP[reading.bar_reading_color] ?? color.text2;
        paHtml = `
          <div style="margin-top:4px;display:flex;align-items:center;gap:6px;">
            <span style="display:inline-block;width:8px;height:8px;background:${dotColor};flex-shrink:0;"></span>
            <span style="color:${dotColor};font-weight:700;">${escapeHtml(reading.pattern)}</span>
          </div>
          <div style="font-size:10px;color:${color.text3};margin-top:2px;">
            ${escapeHtml(reading.bar_summary)}
          </div>`;
      }

      return `<div style="font-family:${font.mono};font-size:12px;width:180px;overflow:hidden;">
  <div style="padding:6px 10px;background:${color.text1};color:${color.bgPage};border-radius:0;">
    <div style="display:flex;justify-content:space-between;align-items:baseline;">
      <b>${fmtAxisTime(bar.time)}</b>
      <span style="font-size:9px;color:${color.text3};">${escapeHtml(timeframe)} &middot; ${state}</span>
    </div>${paHtml}
  </div>
  <div style="padding:8px 10px;line-height:1.9;color:${color.text1};">
    <div>Open <span style="float:right;">${fmtPrice(o)}</span></div>
    <div style="color:${color.teal};">High <span style="float:right;">${fmtPrice(h)}</span></div>
    <div style="color:${color.red};">Low <span style="float:right;">${fmtPrice(l)}</span></div>
    <div>Close <span style="float:right;">${fmtPrice(c)}</span></div>
  </div>
  <div style="padding:6px 10px;border-top:1px dashed ${color.borderSoft};color:${changeColor};font-weight:700;">
    Change <span style="float:right;">${isUp ? '+' : ''}${fmtPrice(change)} (${changePct >= 0 ? '+' : ''}${changePct.toFixed(2)}%)</span>
  </div>
</div>`;
    };

    // Build series array
    const series: EChartsOption['series'] = [
      {
        type: 'candlestick',
        data: itemStyles,
        itemStyle: {
          color: color.teal,
          color0: color.red,
          borderColor: color.tealText,
          borderColor0: color.redText,
        },
        ...(markLineData.length > 0
          ? {
              markLine: {
                symbol: 'none',
                silent: true,
                data: markLineData,
              },
            }
          : {}),
      },
    ];

    if (showPaOverlay && paScatterData.length > 0) {
      series.push({
        type: 'scatter',
        data: paScatterData.filter(Boolean),
        symbol: 'rect',
        symbolSize: [10, 6],
        silent: true,
      } as EChartsOption['series'] extends (infer U)[] ? U : never);
    }

    return {
      animation: false,
      backgroundColor: 'transparent',
      grid: {
        left: 10,
        right: 70,
        top: 20,
        bottom: 60,
        containLabel: false,
      },
      xAxis: {
        type: 'category',
        data: categoryData,
        axisLine: { lineStyle: { color: color.borderHairline } },
        axisTick: { show: false },
        axisLabel: {
          color: color.text2,
          fontSize: 10,
          fontFamily: font.mono,
        },
        axisPointer: {
          label: {
            backgroundColor: color.text1,
            color: color.yellow,
            fontFamily: font.mono,
            fontSize: 10,
          },
        },
      },
      yAxis: {
        type: 'value',
        position: 'right',
        scale: true,
        splitLine: { lineStyle: { color: color.borderSoft, type: 'dashed' } },
        axisLine: { show: false },
        axisTick: { show: false },
        axisLabel: {
          color: color.text2,
          fontSize: 10,
          fontFamily: font.mono,
          formatter: (v: number) => fmtPrice(v),
        },
        axisPointer: {
          label: {
            backgroundColor: color.text1,
            color: color.yellow,
            fontFamily: font.mono,
            fontSize: 10,
          },
        },
      },
      axisPointer: {
        link: [{ xAxisIndex: 'all' }],
        lineStyle: {
          color: color.text3,
          type: 'dashed',
        },
      },
      tooltip: {
        trigger: 'axis',
        show: true,
        backgroundColor: color.bgSurface,
        borderColor: color.borderHairline,
        borderWidth: 1,
        padding: 0,
        formatter: tooltipFormatter as never,
        axisPointer: {
          type: 'cross',
          crossStyle: { color: color.text3 },
        },
      },
      dataZoom: [
        { type: 'inside', xAxisIndex: 0, minValueSpan: 5 },
        {
          type: 'slider',
          xAxisIndex: 0,
          bottom: 10,
          height: 20,
          borderColor: color.borderHairline,
          fillerColor: 'rgba(79,143,232,0.12)',
          handleStyle: { color: color.blue, borderColor: color.blue },
          textStyle: { color: color.text2, fontSize: 10, fontFamily: font.mono },
          dataBackground: {
            lineStyle: { color: color.borderHairline },
            areaStyle: { color: color.borderHairline, opacity: 0.2 },
          },
        },
      ],
      series,
    };
  }, [
    allBars,
    categoryData,
    showPaOverlay,
    barReadings,
    showKeyLevels,
    keyLevels,
    selectedBarIndex,
    readingForIndex,
    timeframe,
  ]);

  /* ---------- events ---------- */

  const onEvents = useMemo(
    () => ({
      click: (params: { seriesType?: string; dataIndex?: number }) => {
        if (params.seriesType === 'candlestick' && params.dataIndex != null) {
          onBarClick(params.dataIndex);
        }
      },
      dblclick: () => {
        const instance = chartRef.current?.getEchartsInstance();
        if (instance) {
          instance.dispatchAction({
            type: 'dataZoom',
            start: 0,
            end: 100,
          });
        }
      },
    }),
    [onBarClick],
  );

  /* ---------- render ---------- */

  return (
    <ReactEChartsCore
      ref={chartRef}
      echarts={echarts}
      option={option}
      style={{ height, width: '100%' }}
      notMerge
      lazyUpdate
      onEvents={onEvents}
    />
  );
}
