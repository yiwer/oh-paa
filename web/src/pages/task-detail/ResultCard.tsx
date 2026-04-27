import styled from 'styled-components';
import { color, font, size, space, border } from '@/theme';
import type { AnalysisResult, AnalysisDeadLetter, AnalysisTask } from '@/api/types';

interface Props {
  result?: AnalysisResult;
  deadLetter?: AnalysisDeadLetter;
  task: AnalysisTask;
}

function extractLabel(outputJson: unknown): string | null {
  if (!outputJson || typeof outputJson !== 'object') return null;
  const obj = outputJson as Record<string, unknown>;
  if (typeof obj.bar_reading_label === 'string') return obj.bar_reading_label;
  return null;
}

function extractPattern(outputJson: unknown): string | null {
  if (!outputJson || typeof outputJson !== 'object') return null;
  const obj = outputJson as Record<string, unknown>;
  if (typeof obj.pattern === 'string') return obj.pattern;
  if (typeof obj.pattern_description === 'string')
    return obj.pattern_description;
  return null;
}

function fmtDatetime(iso: string): string {
  const d = new Date(iso);
  const yyyy = d.getFullYear();
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  const hh = String(d.getHours()).padStart(2, '0');
  const mi = String(d.getMinutes()).padStart(2, '0');
  const ss = String(d.getSeconds()).padStart(2, '0');
  return `${yyyy}-${mm}-${dd} ${hh}:${mi}:${ss}`;
}

export default function ResultCard({ result, deadLetter, task }: Props) {
  if (result) {
    const label = extractLabel(result.output_json);
    const pattern = extractPattern(result.output_json);

    return (
      <Root>
        <Title>Result</Title>

        {(label || pattern) && (
          <Highlight>
            {label && <HighlightLabel>{label}</HighlightLabel>}
            {pattern && <HighlightPattern>{pattern}</HighlightPattern>}
          </Highlight>
        )}

        <KvTable>
          <tbody>
            <Tr>
              <FieldTd>Task ID</FieldTd>
              <ValueTd>{task.id.slice(0, 12)}</ValueTd>
            </Tr>
            <Tr>
              <FieldTd>Schema Version</FieldTd>
              <ValueTd>{task.prompt_version}</ValueTd>
            </Tr>
            <Tr>
              <FieldTd>Created At</FieldTd>
              <ValueTd>{fmtDatetime(result.created_at)}</ValueTd>
            </Tr>
            {task.finished_at && (
              <Tr>
                <FieldTd>Finished At</FieldTd>
                <ValueTd>{fmtDatetime(task.finished_at)}</ValueTd>
              </Tr>
            )}
          </tbody>
        </KvTable>
      </Root>
    );
  }

  if (deadLetter) {
    return (
      <DeadLetterRoot>
        <Title>Dead Letter</Title>
        <KvTable>
          <tbody>
            <Tr>
              <FieldTd>Final Error</FieldTd>
              <ErrorValue>
                {deadLetter.final_error_type}: {deadLetter.final_error_message}
              </ErrorValue>
            </Tr>
            <Tr>
              <FieldTd>Attempts</FieldTd>
              <ValueTd>
                {task.attempt_count}/{task.max_attempts}
              </ValueTd>
            </Tr>
            <Tr>
              <FieldTd>Archived At</FieldTd>
              <ValueTd>{fmtDatetime(deadLetter.created_at)}</ValueTd>
            </Tr>
          </tbody>
        </KvTable>
      </DeadLetterRoot>
    );
  }

  return (
    <Root>
      <Title>Result</Title>
      <Placeholder>Task in progress</Placeholder>
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

const DeadLetterRoot = styled.div`
  background: ${color.bgWhite};
  border: 2px dashed ${color.textLightGray};
  padding: ${space.px16}px;
  font-family: ${font.mono};
  opacity: 0.8;
`;

const Title = styled.h4`
  font-size: ${size.eyebrow}px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 1px;
  color: ${color.textGray};
  margin: 0 0 ${space.px12}px 0;
`;

const Highlight = styled.div`
  display: flex;
  flex-direction: column;
  gap: ${space.px4}px;
  padding: ${space.px8}px ${space.px12}px;
  background: rgba(255, 222, 0, 0.1);
  border-left: 3px solid ${color.yellowPrimary};
  margin-bottom: ${space.px12}px;
`;

const HighlightLabel = styled.span`
  font-size: ${size.body}px;
  font-weight: 700;
  color: ${color.textDark};
`;

const HighlightPattern = styled.span`
  font-size: ${size.bodySm}px;
  color: ${color.textGray};
`;

const KvTable = styled.table`
  width: 100%;
  border-collapse: collapse;
  font-size: ${size.caption}px;
`;

const Tr = styled.tr`
  border-bottom: ${border.dashed};

  &:last-child {
    border-bottom: none;
  }
`;

const FieldTd = styled.td`
  padding: ${space.px4}px 0;
  color: ${color.textGray};
  font-size: ${size.caption}px;
  white-space: nowrap;
`;

const ValueTd = styled.td`
  padding: ${space.px4}px 0;
  text-align: right;
  color: ${color.textDark};
  font-size: ${size.bodyXs}px;
`;

const ErrorValue = styled.td`
  padding: ${space.px4}px 0;
  text-align: right;
  color: ${color.redAccent};
  font-size: ${size.bodyXs}px;
  word-break: break-word;
`;

const Placeholder = styled.div`
  font-size: ${size.bodySm}px;
  color: ${color.textLightGray};
`;
