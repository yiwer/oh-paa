import styled from 'styled-components';
import { color, font, space } from '@/theme';
import type { AnalysisTask } from '@/api/types';

const PIPELINE_STAGES = [
  'shared_pa_state_bar',
  'shared_bar_analysis',
  'shared_daily_context',
] as const;

const STAGE_LABELS: Record<string, string> = {
  shared_pa_state_bar: 'PA State',
  shared_bar_analysis: 'Bar Analysis',
  shared_daily_context: 'Daily Context',
};

type DotVariant = 'succeeded' | 'running' | 'failed' | 'na' | 'cascade';

function resolveDotVariant(
  tasks: AnalysisTask[],
  stage: string,
  upstreamFailed: boolean,
): { variant: DotVariant; attemptLabel?: string } {
  const task = tasks.find((t) => t.task_type === stage);

  if (!task) {
    return upstreamFailed ? { variant: 'cascade' } : { variant: 'na' };
  }

  const status = task.status.toLowerCase();

  if (status === 'succeeded' || status === 'completed') {
    return { variant: 'succeeded' };
  }
  if (status === 'running' || status === 'in_progress') {
    return { variant: 'running' };
  }
  if (status === 'failed' || status === 'dead_letter') {
    return {
      variant: 'failed',
      attemptLabel: `${task.attempt_count}/${task.max_attempts}`,
    };
  }

  return { variant: 'na' };
}

interface Props {
  tasks: AnalysisTask[];
}

export default function PipelineStatusDots({ tasks }: Props) {
  let upstreamFailed = false;

  return (
    <Row>
      {PIPELINE_STAGES.map((stage, i) => {
        const { variant, attemptLabel } = resolveDotVariant(tasks, stage, upstreamFailed);

        if (variant === 'failed' || variant === 'cascade') {
          upstreamFailed = true;
        }

        return (
          <span key={stage} style={{ display: 'inline-flex', alignItems: 'center' }}>
            {i > 0 && <Arrow>{'\u2192'}</Arrow>}
            <DotWrap title={STAGE_LABELS[stage]}>
              <Dot $variant={variant} />
              {attemptLabel && <AttemptCount>{attemptLabel}</AttemptCount>}
            </DotWrap>
          </span>
        );
      })}
    </Row>
  );
}

/* ---- styled ---- */

const Row = styled.div`
  display: inline-flex;
  align-items: center;
  gap: 2px;
`;

const Arrow = styled.span`
  font-size: 10px;
  color: ${color.textLightGray};
  margin: 0 ${space.px4}px;
  user-select: none;
`;

const DotWrap = styled.span`
  display: inline-flex;
  align-items: center;
  gap: 2px;
`;

function dotColor(variant: DotVariant) {
  switch (variant) {
    case 'succeeded': return color.tealAccent;
    case 'running': return color.bluePrimary;
    case 'failed': return color.redAccent;
    case 'na': return 'transparent';
    case 'cascade': return color.textLightGray;
  }
}

function dotBorder(variant: DotVariant) {
  if (variant === 'na') return `1.5px dashed ${color.textLightGray}`;
  return 'none';
}

const Dot = styled.span<{ $variant: DotVariant }>`
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: ${(p) => dotColor(p.$variant)};
  border: ${(p) => dotBorder(p.$variant)};
  flex-shrink: 0;
`;

const AttemptCount = styled.span`
  font-family: ${font.mono};
  font-size: 9px;
  font-weight: 600;
  color: ${color.redAccent};
`;
