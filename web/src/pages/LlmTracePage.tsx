import { useState, useMemo } from 'react';
import styled from 'styled-components';
import { color, font, space } from '@/theme';
import MetricCard, { MetricStrip } from '@/components/MetricCard/MetricCard';
import InstrumentDropdown from '@/components/Dropdown/Dropdown';
import Segmented from '@/components/Segmented/Segmented';
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
        <InstrumentDropdown
          instruments={instruments}
          selectedId={instrumentId}
          onSelect={setSelectedId}
          label="Instrument"
        />
        <Segmented<FilterOption>
          options={[...FILTER_OPTIONS].map((opt) => ({ value: opt, label: opt }))}
          value={filter}
          onChange={setFilter}
          variant="ui"
        />
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

const Title = styled.h1`
  font-family: ${font.ui};
  font-size: 24px;
  font-weight: 700;
  letter-spacing: -0.02em;
  color: ${color.text1};
  margin: 0 0 ${space.px4}px 0;
`;

const Subtitle = styled.p`
  font-family: ${font.ui};
  font-size: 12px;
  color: ${color.text3};
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

const TriggerList = styled.div`
  display: flex;
  flex-direction: column;
  gap: ${space.px8}px;
`;

const EmptyLabel = styled.div`
  font-family: ${font.ui};
  font-size: 13px;
  color: ${color.text3};
  padding: ${space.px20}px 0;
`;
