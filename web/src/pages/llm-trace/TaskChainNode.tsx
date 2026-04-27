import { Link } from 'react-router-dom';
import styled from 'styled-components';
import { color, font, border, space, size } from '@/theme';
import type { AnalysisTask } from '@/api/types';

const TASK_TYPE_LABELS: Record<string, string> = {
  shared_pa_state_bar: 'PA State',
  shared_bar_analysis: 'Bar Analysis',
  shared_daily_context: 'Daily Context',
};

type NodeColor = 'teal' | 'red' | 'blue' | 'deadLetter' | 'gray';

function resolveNodeColor(task: AnalysisTask): NodeColor {
  const status = task.status.toLowerCase();
  if (status === 'succeeded' || status === 'completed') return 'teal';
  if (status === 'dead_letter') return 'deadLetter';
  if (status === 'failed') return 'red';
  if (status === 'running' || status === 'in_progress') return 'blue';
  return 'gray';
}

function nodeIcon(task: AnalysisTask): string {
  const status = task.status.toLowerCase();
  if (status === 'succeeded' || status === 'completed') return '\u2713';
  if (status === 'dead_letter') return '\u2620';
  if (status === 'failed') return '\u2717';
  return '\u2026';
}

function formatLatency(task: AnalysisTask): string | null {
  if (!task.started_at || !task.finished_at) return null;
  const ms = new Date(task.finished_at).getTime() - new Date(task.started_at).getTime();
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function outputSummary(task: AnalysisTask): string {
  const status = task.status.toLowerCase();
  if (status === 'failed' || status === 'dead_letter') {
    return task.last_error_message ?? task.last_error_code ?? 'Unknown error';
  }
  if (status === 'succeeded' || status === 'completed') {
    return 'Completed successfully';
  }
  if (status === 'running' || status === 'in_progress') {
    return 'Running...';
  }
  return task.status;
}

interface Props {
  task: AnalysisTask;
  isLast: boolean;
}

export default function TaskChainNode({ task, isLast }: Props) {
  const nodeColor = resolveNodeColor(task);
  const icon = nodeIcon(task);
  const latency = formatLatency(task);
  const label = TASK_TYPE_LABELS[task.task_type] ?? task.task_type;
  const summary = outputSummary(task);

  return (
    <Row>
      <Timeline>
        <NodeCircle $color={nodeColor}>{icon}</NodeCircle>
        {!isLast && <VerticalLine />}
      </Timeline>

      <TaskCard $color={nodeColor}>
        <CardHeader>
          <TaskName>{label}</TaskName>
          <StatusBadge $color={nodeColor}>{task.status}</StatusBadge>
        </CardHeader>

        <CardMeta>
          <span>{task.prompt_key}</span>
          {latency && <span>{latency}</span>}
          <span>
            {task.attempt_count}/{task.max_attempts} attempts
          </span>
        </CardMeta>

        <CardSummary $isError={nodeColor === 'red' || nodeColor === 'deadLetter'}>
          {summary}
        </CardSummary>

        <DetailLink to={`/llm-trace/${task.id}`}>{'\u2192'} detail</DetailLink>
      </TaskCard>
    </Row>
  );
}

/* ---- styled ---- */

const Row = styled.div`
  display: flex;
  gap: ${space.px12}px;
`;

const Timeline = styled.div`
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 16px;
  flex-shrink: 0;
`;

function circleBackground(c: NodeColor) {
  switch (c) {
    case 'teal': return color.tealAccent;
    case 'red': return color.redAccent;
    case 'blue': return color.bluePrimary;
    case 'deadLetter': return color.textLightGray;
    case 'gray': return color.textLightGray;
  }
}

function circleBorder(c: NodeColor) {
  if (c === 'deadLetter') return `2px solid ${color.redAccent}`;
  return 'none';
}

const NodeCircle = styled.div<{ $color: NodeColor }>`
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: ${(p) => circleBackground(p.$color)};
  border: ${(p) => circleBorder(p.$color)};
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 9px;
  font-weight: 700;
  color: ${color.bgWhite};
  flex-shrink: 0;
`;

const VerticalLine = styled.div`
  width: 2px;
  flex: 1;
  min-height: 12px;
  background: ${color.bgLightGray};
`;

function cardBorder(c: NodeColor) {
  if (c === 'red' || c === 'deadLetter') return `2px solid ${color.redAccent}`;
  return border.thin;
}

const TaskCard = styled.div<{ $color: NodeColor }>`
  flex: 1;
  background: ${color.bgWhite};
  border: ${(p) => cardBorder(p.$color)};
  padding: ${space.px10}px ${space.px12}px;
  margin-bottom: ${space.px8}px;
`;

const CardHeader = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px8}px;
  margin-bottom: ${space.px4}px;
`;

const TaskName = styled.span`
  font-family: ${font.mono};
  font-size: ${size.bodySm}px;
  font-weight: 700;
  color: ${color.textDark};
`;

function badgeBackground(c: NodeColor) {
  switch (c) {
    case 'teal': return color.tealAccent;
    case 'red': return color.redAccent;
    case 'blue': return color.bluePrimary;
    case 'deadLetter': return color.redAccent;
    case 'gray': return color.bgLightGray;
  }
}

function badgeColor(c: NodeColor) {
  if (c === 'gray') return color.textDark;
  return color.bgWhite;
}

const StatusBadge = styled.span<{ $color: NodeColor }>`
  font-family: ${font.mono};
  font-size: 9px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  padding: 1px ${space.px6}px;
  background: ${(p) => badgeBackground(p.$color)};
  color: ${(p) => badgeColor(p.$color)};
`;

const CardMeta = styled.div`
  display: flex;
  gap: ${space.px10}px;
  font-family: ${font.mono};
  font-size: ${size.bodyXs}px;
  color: ${color.textGray};
  margin-bottom: ${space.px4}px;
`;

const CardSummary = styled.div<{ $isError: boolean }>`
  font-size: ${size.bodyXs}px;
  color: ${(p) => (p.$isError ? color.redAccent : color.textGray)};
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-bottom: ${space.px4}px;
`;

const DetailLink = styled(Link)`
  font-family: ${font.mono};
  font-size: ${size.caption}px;
  font-weight: 600;
  color: ${color.bluePrimary};
  text-decoration: none;

  &:hover {
    text-decoration: underline;
  }
`;
