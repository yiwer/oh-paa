import { useState } from 'react';
import styled from 'styled-components';
import { color, font, border, radius, shadow, space } from '@/theme';
import type { AnalysisTask } from '@/api/types';
import PipelineStatusDots from './PipelineStatusDots';
import TaskChainNode from './TaskChainNode';

function hasError(tasks: AnalysisTask[]) {
  return tasks.some((t) => {
    const s = t.status.toLowerCase();
    return s === 'failed' || s === 'dead_letter';
  });
}

function totalLatency(tasks: AnalysisTask[]): string | null {
  let earliest: number | null = null;
  let latest: number | null = null;

  for (const t of tasks) {
    if (t.started_at) {
      const s = new Date(t.started_at).getTime();
      if (earliest === null || s < earliest) earliest = s;
    }
    if (t.finished_at) {
      const f = new Date(t.finished_at).getTime();
      if (latest === null || f > latest) latest = f;
    }
  }

  if (earliest === null || latest === null) return null;
  const ms = latest - earliest;
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function statusText(tasks: AnalysisTask[]): string {
  if (tasks.some((t) => t.status.toLowerCase() === 'running' || t.status.toLowerCase() === 'in_progress')) {
    return 'In Progress';
  }
  if (hasError(tasks)) return 'Error';
  if (tasks.every((t) => t.status.toLowerCase() === 'succeeded' || t.status.toLowerCase() === 'completed')) {
    return 'Passed';
  }
  return 'Pending';
}

function extractTimeframe(tasks: AnalysisTask[]): string {
  for (const t of tasks) {
    if (t.timeframe) return t.timeframe;
  }
  return '--';
}

function extractBarState(tasks: AnalysisTask[]): string {
  for (const t of tasks) {
    if (t.bar_state) return t.bar_state;
  }
  return '';
}

function extractBarCloseTime(tasks: AnalysisTask[]): string {
  for (const t of tasks) {
    if (t.bar_close_time) return t.bar_close_time;
  }
  return '--';
}

/** Pipeline ordering for task chain display */
const STAGE_ORDER: Record<string, number> = {
  shared_pa_state_bar: 0,
  shared_bar_analysis: 1,
  shared_daily_context: 2,
};

function sortedTasks(tasks: AnalysisTask[]) {
  return [...tasks].sort(
    (a, b) => (STAGE_ORDER[a.task_type] ?? 99) - (STAGE_ORDER[b.task_type] ?? 99),
  );
}

type AccentState = 'error' | 'passed' | 'progress' | 'pending';

function accentState(tasks: AnalysisTask[]): AccentState {
  if (hasError(tasks)) return 'error';
  if (
    tasks.some(
      (t) =>
        t.status.toLowerCase() === 'running' ||
        t.status.toLowerCase() === 'in_progress',
    )
  ) {
    return 'progress';
  }
  if (
    tasks.length > 0 &&
    tasks.every(
      (t) =>
        t.status.toLowerCase() === 'succeeded' ||
        t.status.toLowerCase() === 'completed',
    )
  ) {
    return 'passed';
  }
  return 'pending';
}

interface Props {
  tasks: AnalysisTask[];
  triggerKey: string;
}

export default function TriggerRow({ tasks, triggerKey: _triggerKey }: Props) {
  const [expanded, setExpanded] = useState(false);
  const accent = accentState(tasks);
  const latency = totalLatency(tasks);
  const status = statusText(tasks);
  const timeframe = extractTimeframe(tasks);
  const barCloseTime = extractBarCloseTime(tasks);
  const barState = extractBarState(tasks);
  const ordered = sortedTasks(tasks);

  return (
    <Card $accent={accent}>
      <Header onClick={() => setExpanded((v) => !v)}>
        <Left>
          <TimeframeBadge>{timeframe}</TimeframeBadge>
          <BarCloseTime>{barCloseTime}</BarCloseTime>
          {barState && <BarState>{barState}</BarState>}
        </Left>

        <Right>
          <PipelineStatusDots tasks={tasks} />
          <LatencyText>{latency ?? status}</LatencyText>
          <Arrow $expanded={expanded}>{'\u25B6'}</Arrow>
        </Right>
      </Header>

      {expanded && (
        <ExpandedSection>
          {ordered.map((task, i) => (
            <TaskChainNode key={task.id} task={task} isLast={i === ordered.length - 1} />
          ))}
        </ExpandedSection>
      )}
    </Card>
  );
}

/* ---- styled ---- */

const Card = styled.div<{ $accent: AccentState }>`
  position: relative;
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: ${radius.card};
  box-shadow: ${shadow.card};
  margin-bottom: ${space.px8}px;
  overflow: hidden;

  &::before {
    content: '';
    position: absolute;
    left: 0;
    top: 12px;
    bottom: 12px;
    width: 3px;
    border-radius: 0 2px 2px 0;
    background: ${(p) => {
      switch (p.$accent) {
        case 'error': return color.red;
        case 'passed': return color.teal;
        case 'progress': return color.blue;
        case 'pending': return color.text3;
      }
    }};
  }
`;

const Header = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: ${space.px10}px ${space.px16}px;
  cursor: pointer;
  user-select: none;
`;

const Left = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px8}px;
`;

const Right = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px12}px;
`;

const TimeframeBadge = styled.span`
  font-family: ${font.mono};
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  padding: 2px ${space.px6}px;
  border-radius: ${radius.tag};
  background: ${color.text1};
  color: ${color.yellow};
  text-transform: uppercase;
`;

const BarCloseTime = styled.span`
  font-family: ${font.mono};
  font-size: 13px;
  font-weight: 700;
  color: ${color.text1};
`;

const BarState = styled.span`
  font-family: ${font.mono};
  font-size: 12px;
  color: ${color.text2};
`;

const LatencyText = styled.span`
  font-family: ${font.mono};
  font-size: 12px;
  color: ${color.text2};
  min-width: 60px;
  text-align: right;
`;

const Arrow = styled.span<{ $expanded: boolean }>`
  font-size: 10px;
  color: ${color.text3};
  transition: transform 0.15s ease;
  transform: rotate(${(p) => (p.$expanded ? '90deg' : '0deg')});
`;

const ExpandedSection = styled.div`
  padding: ${space.px12}px ${space.px16}px;
  border-top: 1px dashed ${color.borderSoft};
`;
