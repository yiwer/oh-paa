import { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import styled from 'styled-components';
import { color, font, size, space, border } from '@/theme';
import { useTask, useAttempts, useResult, useDeadLetter } from '@/api/hooks/useLlmTrace';
import TimelineCard from '@/pages/task-detail/TimelineCard';
import PromptInputCard from '@/pages/task-detail/PromptInputCard';
import LlmResponseCard from '@/pages/task-detail/LlmResponseCard';
import ValidationCard from '@/pages/task-detail/ValidationCard';
import ResultCard from '@/pages/task-detail/ResultCard';

const TASK_TYPE_LABELS: Record<string, string> = {
  shared_pa_state_bar: 'PA State',
  shared_bar_analysis: 'Bar Analysis',
  shared_daily_context: 'Daily Context',
};

function statusColor(status: string): string {
  const s = status.toLowerCase();
  if (s === 'succeeded' || s === 'completed') return color.tealAccent;
  if (s === 'failed' || s === 'dead_letter') return color.redAccent;
  if (s === 'running' || s === 'in_progress') return color.bluePrimary;
  return color.textLightGray;
}

function formatLatency(startedAt: string | null, finishedAt: string | null): string | null {
  if (!startedAt || !finishedAt) return null;
  const ms = new Date(finishedAt).getTime() - new Date(startedAt).getTime();
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function attemptLatency(a: { started_at: string; finished_at: string | null }): string {
  if (!a.finished_at) return 'running';
  const ms = new Date(a.finished_at).getTime() - new Date(a.started_at).getTime();
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

export default function TaskDetailPage() {
  const { taskId } = useParams<{ taskId: string }>();
  const { data: task } = useTask(taskId!);
  const { data: attempts } = useAttempts(taskId!);
  const { data: result } = useResult(taskId!);
  const { data: deadLetter } = useDeadLetter(taskId!);

  const [selectedAttemptIndex, setSelectedAttemptIndex] = useState(0);
  const attempt = attempts?.[selectedAttemptIndex];

  const label = task ? (TASK_TYPE_LABELS[task.task_type] ?? task.task_type) : '';
  const latency = task ? formatLatency(task.started_at, task.finished_at) : null;

  const isDead = task?.status.toLowerCase() === 'dead_letter';
  const hasError = attempt?.error_type != null;

  return (
    <Root>
      {/* Breadcrumb */}
      <Breadcrumb>
        <BreadcrumbLink to="/llm-trace">{'\u2190'} LLM Trace</BreadcrumbLink>
        <BreadcrumbSep>/</BreadcrumbSep>
        <BreadcrumbCurrent>{label || 'Task Detail'}</BreadcrumbCurrent>
      </Breadcrumb>

      {/* Header card */}
      {task && (
        <HeaderCard>
          <HeaderRow>
            <TaskType>{label}</TaskType>
            <StatusBadge style={{ background: statusColor(task.status) }}>
              {task.status}
            </StatusBadge>
          </HeaderRow>
          <HeaderMeta>
            {attempt && <span>{attempt.llm_provider}</span>}
            {latency && <span>{latency}</span>}
            <span>
              {task.attempt_count}/{task.max_attempts} attempts
            </span>
          </HeaderMeta>
        </HeaderCard>
      )}

      {/* Attempt tabs */}
      {attempts && attempts.length > 1 && (
        <TabRow>
          {attempts.map((a, i) => (
            <AttemptTab
              key={a.id}
              $active={i === selectedAttemptIndex}
              onClick={() => setSelectedAttemptIndex(i)}
            >
              Attempt {a.attempt_number} &middot; {attemptLatency(a)} &middot;{' '}
              {a.error_type ? a.error_type : 'success'}
            </AttemptTab>
          ))}
        </TabRow>
      )}

      {/* Timeline */}
      {task && (
        <TimelineColumn>
          <TimelineCard
            icon={'\u2192'}
            bgColor={color.darkSurface}
            iconColor={color.bgWhite}
          >
            <PromptInputCard task={task} />
          </TimelineCard>

          {attempt && (
            <TimelineCard
              icon={'\u2190'}
              bgColor={color.darkSurface}
              iconColor={color.bgWhite}
            >
              <LlmResponseCard attempt={attempt} />
            </TimelineCard>
          )}

          {attempt && (
            <TimelineCard
              icon={hasError ? '\u2717' : '\u2713'}
              bgColor={hasError ? color.redAccent : color.tealAccent}
              iconColor={color.bgWhite}
            >
              <ValidationCard attempt={attempt} task={task} />
            </TimelineCard>
          )}

          <TimelineCard
            icon={isDead ? '\u2620' : '\u25CE'}
            bgColor={isDead ? color.darkSurface : color.yellowPrimary}
            iconColor={isDead ? color.redAccent : color.textDark}
            isLast
          >
            <ResultCard result={result} deadLetter={deadLetter} task={task} />
          </TimelineCard>
        </TimelineColumn>
      )}

      {!task && <Loading>Loading task...</Loading>}
    </Root>
  );
}

/* ---- styled ---- */

const Root = styled.div`
  display: flex;
  flex-direction: column;
`;

const Breadcrumb = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px6}px;
  margin-bottom: ${space.px16}px;
`;

const BreadcrumbLink = styled(Link)`
  font-family: ${font.mono};
  font-size: ${size.bodySm}px;
  font-weight: 700;
  color: ${color.bluePrimary};
  text-decoration: none;

  &:hover {
    text-decoration: underline;
  }
`;

const BreadcrumbSep = styled.span`
  font-size: ${size.bodySm}px;
  color: ${color.textLightGray};
`;

const BreadcrumbCurrent = styled.span`
  font-family: ${font.mono};
  font-size: ${size.bodySm}px;
  font-weight: 700;
  color: ${color.textDark};
`;

const HeaderCard = styled.div`
  background: ${color.bgWhite};
  border: ${border.std};
  padding: ${space.px16}px;
  margin-bottom: ${space.px16}px;
`;

const HeaderRow = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px10}px;
  margin-bottom: ${space.px6}px;
`;

const TaskType = styled.span`
  font-family: ${font.mono};
  font-size: 18px;
  font-weight: 800;
  color: ${color.textDark};
`;

const StatusBadge = styled.span`
  font-family: ${font.mono};
  font-size: 9px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  padding: 2px ${space.px8}px;
  color: ${color.bgWhite};
`;

const HeaderMeta = styled.div`
  display: flex;
  gap: ${space.px10}px;
  font-family: ${font.mono};
  font-size: ${size.bodyXs}px;
  color: ${color.textGray};
`;

const TabRow = styled.div`
  display: flex;
  gap: 0;
  margin-bottom: ${space.px16}px;
`;

const AttemptTab = styled.button<{ $active: boolean }>`
  all: unset;
  cursor: pointer;
  font-family: ${font.mono};
  font-size: ${size.caption}px;
  font-weight: 700;
  padding: ${space.px6}px ${space.px12}px;
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

const TimelineColumn = styled.div`
  display: flex;
  flex-direction: column;
`;

const Loading = styled.div`
  font-family: ${font.mono};
  font-size: ${size.bodySm}px;
  color: ${color.textLightGray};
  padding: ${space.px20}px 0;
`;
