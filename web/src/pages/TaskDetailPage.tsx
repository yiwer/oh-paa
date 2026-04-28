import { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import styled from 'styled-components';
import { color, font, border, space } from '@/theme';
import { useTask, useAttempts, useResult, useDeadLetter } from '@/api/hooks/useLlmTrace';
import StatusPill, { type StatusVariant } from '@/components/StatusPill/StatusPill';
import Segmented from '@/components/Segmented/Segmented';
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

function statusVariant(status: string): StatusVariant {
  const s = status.toLowerCase();
  if (s === 'succeeded' || s === 'completed') return 'ok';
  if (s === 'running' || s === 'in_progress') return 'info';
  if (s === 'failed' || s === 'dead_letter') return 'err';
  return 'neutral';
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
            <StatusPill variant={statusVariant(task.status)}>{task.status}</StatusPill>
            <KLineLink
              to={`/kline?instrument=${task.instrument_id}${task.timeframe ? `&timeframe=${task.timeframe}` : ''}`}
            >
              View in K-Line &rarr;
            </KLineLink>
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
        <AttemptTabsWrap>
          <Segmented
            options={attempts.map((a, i) => ({
              value: String(i),
              label: `Attempt ${a.attempt_number} · ${attemptLatency(a)} · ${a.error_type ?? 'success'}`,
            }))}
            value={String(selectedAttemptIndex)}
            onChange={(v) => setSelectedAttemptIndex(Number(v))}
            variant="ui"
          />
        </AttemptTabsWrap>
      )}

      {/* Timeline */}
      {task && (
        <TimelineColumn>
          <TimelineCard
            icon={'\u2192'}
            bgColor={color.text1}
            iconColor={color.bgSurface}
          >
            <PromptInputCard task={task} />
          </TimelineCard>

          {attempt && (
            <TimelineCard
              icon={'\u2190'}
              bgColor={color.text1}
              iconColor={color.bgSurface}
            >
              <LlmResponseCard attempt={attempt} />
            </TimelineCard>
          )}

          {attempt && (
            <TimelineCard
              icon={hasError ? '\u2717' : '\u2713'}
              bgColor={hasError ? color.red : color.teal}
              iconColor={color.bgSurface}
            >
              <ValidationCard attempt={attempt} task={task} />
            </TimelineCard>
          )}

          <TimelineCard
            icon={isDead ? '\u2620' : '\u25CE'}
            bgColor={isDead ? color.text1 : color.yellow}
            iconColor={isDead ? color.red : color.text1}
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
  font-size: 12px;
  font-weight: 600;
  color: ${color.blueText};
  text-decoration: none;

  &:hover {
    text-decoration: underline;
  }
`;

const BreadcrumbSep = styled.span`
  font-size: 12px;
  color: ${color.textDisabled};
`;

const BreadcrumbCurrent = styled.span`
  font-family: ${font.mono};
  font-size: 12px;
  font-weight: 600;
  color: ${color.text1};
`;

const HeaderCard = styled.div`
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: 10px;
  box-shadow: 0 1px 2px rgba(28,25,20,.04);
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
  font-size: 16px;
  font-weight: 700;
  color: ${color.text1};
`;

const KLineLink = styled(Link)`
  margin-left: auto;
  font-family: ${font.mono};
  font-size: 11px;
  font-weight: 600;
  color: ${color.blueText};
  text-decoration: none;

  &:hover {
    text-decoration: underline;
  }
`;

const HeaderMeta = styled.div`
  display: flex;
  gap: ${space.px10}px;
  font-family: ${font.mono};
  font-size: 11px;
  color: ${color.text2};
`;

const AttemptTabsWrap = styled.div`
  margin-bottom: ${space.px16}px;
`;

const TimelineColumn = styled.div`
  display: flex;
  flex-direction: column;
`;

const Loading = styled.div`
  font-family: ${font.ui};
  font-size: 13px;
  color: ${color.text3};
  padding: ${space.px20}px 0;
`;
