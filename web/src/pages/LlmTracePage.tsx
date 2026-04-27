import { useState, useMemo } from 'react';
import styled from 'styled-components';
import { color, font, border, space, size } from '@/theme';
import MetricCard, { MetricStrip } from '@/components/MetricCard/MetricCard';
import InstrumentSwitcher from '@/components/InstrumentSwitcher/InstrumentSwitcher';
import { useInstruments } from '@/api/hooks/usePipeline';
import { useTasks } from '@/api/hooks/useLlmTrace';
import TriggerRow from '@/pages/llm-trace/TriggerRow';
import type { AnalysisTask } from '@/api/types';

const FILTER_OPTIONS = ['All', 'PA State', 'Bar Analysis', 'Daily Context'] as const;
type FilterOption = (typeof FILTER_OPTIONS)[number];

const FILTER_TO_TASK_TYPE: Record<FilterOption, string | null> = {
  All: null,
  'PA State': 'shared_pa_state_bar',
  'Bar Analysis': 'shared_bar_analysis',
  'Daily Context': 'shared_daily_context',
};

function groupByTrigger(tasks: AnalysisTask[]) {
  const triggers = new Map<string, AnalysisTask[]>();
  for (const task of tasks) {
    const key = `${task.timeframe ?? '--'}:${task.bar_close_time ?? '--'}`;
    if (!triggers.has(key)) triggers.set(key, []);
    triggers.get(key)!.push(task);
  }
  // Sort triggers by bar_close_time descending
  return [...triggers.entries()].sort((a, b) => {
    const aTime = a[1][0]?.bar_close_time ?? '';
    const bTime = b[1][0]?.bar_close_time ?? '';
    return bTime.localeCompare(aTime);
  });
}

function isToday(dateStr: string | null): boolean {
  if (!dateStr) return false;
  const d = new Date(dateStr);
  const now = new Date();
  return (
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate()
  );
}

function computeMetrics(tasks: AnalysisTask[]) {
  const todayTriggers = new Set<string>();
  let passed = 0;
  let inProgress = 0;
  let errors = 0;
  let totalLatencyMs = 0;
  let latencyCount = 0;

  for (const t of tasks) {
    if (isToday(t.bar_close_time) || isToday(t.started_at)) {
      todayTriggers.add(`${t.timeframe}:${t.bar_close_time}`);
    }

    const status = t.status.toLowerCase();
    if (status === 'succeeded' || status === 'completed') {
      passed++;
    } else if (status === 'running' || status === 'in_progress') {
      inProgress++;
    } else if (status === 'failed' || status === 'dead_letter') {
      errors++;
    }

    if (t.started_at && t.finished_at) {
      const ms = new Date(t.finished_at).getTime() - new Date(t.started_at).getTime();
      if (ms > 0) {
        totalLatencyMs += ms;
        latencyCount++;
      }
    }
  }

  const avgLatency =
    latencyCount > 0
      ? `${(totalLatencyMs / latencyCount / 1000).toFixed(1)}s`
      : '\u2014';

  return {
    triggers: todayTriggers.size,
    passed,
    inProgress,
    errors,
    avgLatency,
  };
}

export default function LlmTracePage() {
  const { data: instruments = [] } = useInstruments();
  const [selectedId, setSelectedId] = useState('');
  const [filter, setFilter] = useState<FilterOption>('All');

  // Auto-select first instrument
  const instrumentId = selectedId || instruments[0]?.id || '';

  const { data: allTasks = [] } = useTasks(instrumentId);

  const filteredTasks = useMemo(() => {
    const taskType = FILTER_TO_TASK_TYPE[filter];
    if (!taskType) return allTasks;
    return allTasks.filter((t) => t.task_type === taskType);
  }, [allTasks, filter]);

  const triggers = useMemo(() => groupByTrigger(filteredTasks), [filteredTasks]);
  const metrics = useMemo(() => computeMetrics(allTasks), [allTasks]);

  return (
    <Root>
      <Title>LLM Trace</Title>
      <Subtitle>{'LLM 分析任务链路追踪'}</Subtitle>

      <ControlBar>
        <InstrumentSwitcher
          instruments={instruments}
          selectedId={instrumentId}
          onSelect={setSelectedId}
        />
        <FilterRow>
          {FILTER_OPTIONS.map((opt) => (
            <FilterPill
              key={opt}
              $active={filter === opt}
              onClick={() => setFilter(opt)}
            >
              {opt}
            </FilterPill>
          ))}
        </FilterRow>
      </ControlBar>

      <MetricStrip style={{ marginBottom: space.px24 }}>
        <MetricCard accent="teal" eyebrow="Triggers (today)" value={metrics.triggers} />
        <MetricCard accent="blue" eyebrow="Passed" value={metrics.passed} />
        <MetricCard accent="yellow" eyebrow="In Progress" value={metrics.inProgress} />
        <MetricCard accent="red" eyebrow="Errors" value={metrics.errors} />
        <MetricCard accent="gray" eyebrow="Avg Latency" value={metrics.avgLatency} />
      </MetricStrip>

      <TriggerList>
        {triggers.length === 0 && (
          <EmptyLabel>
            {instrumentId ? 'No analysis tasks found' : 'Select an instrument'}
          </EmptyLabel>
        )}
        {triggers.map(([key, tasks]) => (
          <TriggerRow key={key} tasks={tasks} triggerKey={key} />
        ))}
      </TriggerList>
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

const ControlBar = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: ${space.px12}px;
  margin-bottom: ${space.px16}px;
`;

const FilterRow = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px4}px;
`;

const FilterPill = styled.button<{ $active: boolean }>`
  all: unset;
  cursor: pointer;
  font-family: ${font.mono};
  font-size: 11px;
  font-weight: 700;
  padding: ${space.px4}px ${space.px8}px;
  border: ${border.std};
  border-radius: 0px;
  background: ${(p) => (p.$active ? color.textDark : color.bgWhite)};
  color: ${(p) => (p.$active ? color.yellowPrimary : color.textDark)};
  transition: background-color 0.15s, color 0.15s;

  &:hover {
    background: ${(p) => (p.$active ? color.textDark : color.bgLightGray)};
  }
`;

const TriggerList = styled.div`
  display: flex;
  flex-direction: column;
`;

const EmptyLabel = styled.div`
  font-size: ${size.bodySm}px;
  color: ${color.textLightGray};
  padding: ${space.px20}px 0;
`;
