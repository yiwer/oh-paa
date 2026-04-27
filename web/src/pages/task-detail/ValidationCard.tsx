import styled from 'styled-components';
import { color, font, size, space, border } from '@/theme';
import type { AnalysisAttempt, AnalysisTask } from '@/api/types';

interface Props {
  attempt: AnalysisAttempt;
  task: AnalysisTask;
}

function isRetryable(errorType: string | null): boolean {
  if (!errorType) return false;
  const lower = errorType.toLowerCase();
  return (
    lower.includes('timeout') ||
    lower.includes('rate_limit') ||
    lower.includes('ratelimit') ||
    lower.includes('server') ||
    lower.includes('transient')
  );
}

function retryDecision(attempt: AnalysisAttempt, task: AnalysisTask): string {
  if (!attempt.error_type) return '--';
  if (attempt.attempt_number >= task.max_attempts) return 'Max attempts reached';
  if (isRetryable(attempt.error_type)) return 'Will retry';
  return 'Non-retryable, stopping';
}

export default function ValidationCard({ attempt, task }: Props) {
  const hasError = !!attempt.error_type;

  if (!hasError) {
    return (
      <Root>
        <Title>Validation</Title>
        <SuccessRow>
          <CheckMark>{'\u2713'}</CheckMark>
          <SuccessText>Schema Pass</SuccessText>
        </SuccessRow>
      </Root>
    );
  }

  return (
    <Root>
      <Title>Validation</Title>
      <ErrorTable>
        <tbody>
          <ErrorTr>
            <FieldTd>Error Type</FieldTd>
            <ValueTd>{attempt.error_type}</ValueTd>
          </ErrorTr>
          <ErrorTr>
            <FieldTd>Message</FieldTd>
            <ValueTd>{attempt.error_message ?? '--'}</ValueTd>
          </ErrorTr>
          <ErrorTr>
            <FieldTd>Retryable</FieldTd>
            <ValueTd>{isRetryable(attempt.error_type) ? 'Yes' : 'No'}</ValueTd>
          </ErrorTr>
          <ErrorTr>
            <FieldTd>Retry Decision</FieldTd>
            <ValueTd>{retryDecision(attempt, task)}</ValueTd>
          </ErrorTr>
        </tbody>
      </ErrorTable>
    </Root>
  );
}

/* ---- styled ---- */

const Root = styled.div`
  background: ${color.bgWhite};
  border: ${border.std};
  padding: ${space.px16}px;
  font-family: ${font.mono};
`;

const Title = styled.h4`
  font-size: ${size.eyebrow}px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 1px;
  color: ${color.textGray};
  margin: 0 0 ${space.px12}px 0;
`;

const SuccessRow = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px8}px;
`;

const CheckMark = styled.span`
  font-size: ${size.bodyLg}px;
  font-weight: 700;
  color: ${color.tealAccent};
`;

const SuccessText = styled.span`
  font-size: ${size.bodySm}px;
  font-weight: 700;
  color: ${color.tealAccent};
`;

const ErrorTable = styled.table`
  width: 100%;
  border-collapse: collapse;
  font-size: ${size.caption}px;
  border: 1px solid ${color.redAccent};
`;

const ErrorTr = styled.tr`
  border-bottom: ${border.dashed};

  &:last-child {
    border-bottom: none;
  }
`;

const FieldTd = styled.td`
  padding: ${space.px4}px ${space.px8}px;
  color: ${color.textGray};
  font-size: ${size.caption}px;
  white-space: nowrap;
`;

const ValueTd = styled.td`
  padding: ${space.px4}px ${space.px8}px;
  text-align: right;
  color: ${color.textDark};
  font-size: ${size.bodyXs}px;
  word-break: break-word;
`;
