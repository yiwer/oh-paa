import styled from 'styled-components';
import { color, font, size, space, border } from '@/theme';
import type { AnalysisAttempt } from '@/api/types';

interface Props {
  attempt: AnalysisAttempt;
}

function computeLatency(attempt: AnalysisAttempt): string | null {
  if (!attempt.finished_at) return null;
  const ms =
    new Date(attempt.finished_at).getTime() -
    new Date(attempt.started_at).getTime();
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function deriveHttpStatus(errorType: string | null): string {
  if (!errorType) return '200';
  const lower = errorType.toLowerCase();
  if (lower.includes('timeout')) return '408';
  if (lower.includes('rate_limit') || lower.includes('ratelimit')) return '429';
  if (lower.includes('auth')) return '401';
  if (lower.includes('server') || lower.includes('internal')) return '500';
  if (lower.includes('bad_request') || lower.includes('validation')) return '400';
  return 'ERR';
}

function extractReasoning(raw: unknown): string | null {
  if (!raw || typeof raw !== 'object') return null;
  const obj = raw as Record<string, unknown>;

  // Check for reasoning_content or thinking field
  if (typeof obj.reasoning_content === 'string') return obj.reasoning_content;
  if (typeof obj.thinking === 'string') return obj.thinking;

  // Check in choices array (OpenAI-compatible format)
  const choices = obj.choices;
  if (Array.isArray(choices) && choices.length > 0) {
    const msg = (choices[0] as Record<string, unknown>).message as
      | Record<string, unknown>
      | undefined;
    if (msg && typeof msg.reasoning_content === 'string')
      return msg.reasoning_content;
  }

  return null;
}

function safeEntries(value: unknown): [string, unknown][] {
  if (!value || typeof value !== 'object') return [];
  return Object.entries(value as Record<string, unknown>);
}

export default function LlmResponseCard({ attempt }: Props) {
  const latency = computeLatency(attempt);
  const httpStatus = deriveHttpStatus(attempt.error_type);
  const reasoning = extractReasoning(attempt.raw_response_json);
  const outputEntries = safeEntries(attempt.parsed_output_json);

  return (
    <Root>
      <Title>LLM Response</Title>

      {/* Meta row */}
      <MetaRow>
        <MetaItem>
          <MetaLabel>Provider</MetaLabel>
          <MetaValue>{attempt.llm_provider}</MetaValue>
        </MetaItem>
        <MetaItem>
          <MetaLabel>Model</MetaLabel>
          <MetaValue>{attempt.model}</MetaValue>
        </MetaItem>
        {latency && (
          <MetaItem>
            <MetaLabel>Latency</MetaLabel>
            <MetaValue>{latency}</MetaValue>
          </MetaItem>
        )}
        <MetaItem>
          <MetaLabel>HTTP Status</MetaLabel>
          <MetaValue $isError={httpStatus !== '200'}>{httpStatus}</MetaValue>
        </MetaItem>
      </MetaRow>

      {/* Reasoning chain */}
      {reasoning && (
        <ReasoningDetails>
          <summary>Reasoning chain</summary>
          <ReasoningContent>{reasoning}</ReasoningContent>
        </ReasoningDetails>
      )}

      {/* Output fields */}
      {outputEntries.length > 0 && (
        <>
          <SectionLabel>Parsed Output</SectionLabel>
          <KvTable>
            <tbody>
              {outputEntries.map(([key, val]) => (
                <Tr key={key}>
                  <FieldTd>{key}</FieldTd>
                  <ValueTd>
                    {val == null ? (
                      <MissingTag>&#9888; MISSING</MissingTag>
                    ) : typeof val === 'object' ? (
                      JSON.stringify(val)
                    ) : (
                      String(val)
                    )}
                  </ValueTd>
                </Tr>
              ))}
            </tbody>
          </KvTable>
        </>
      )}

      {outputEntries.length === 0 && !attempt.error_type && (
        <Placeholder>No parsed output</Placeholder>
      )}

      {attempt.error_type && (
        <ErrorBox>
          {attempt.error_type}: {attempt.error_message ?? 'Unknown error'}
        </ErrorBox>
      )}
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

const MetaRow = styled.div`
  display: flex;
  gap: ${space.px16}px;
  flex-wrap: wrap;
  margin-bottom: ${space.px12}px;
  padding-bottom: ${space.px8}px;
  border-bottom: ${border.dashed};
`;

const MetaItem = styled.div`
  display: flex;
  flex-direction: column;
  gap: 2px;
`;

const MetaLabel = styled.span`
  font-size: ${size.mini}px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: ${color.textLightGray};
`;

const MetaValue = styled.span<{ $isError?: boolean }>`
  font-size: ${size.bodyXs}px;
  font-weight: 700;
  color: ${(p) => (p.$isError ? color.redAccent : color.textDark)};
`;

const ReasoningDetails = styled.details`
  margin-bottom: ${space.px12}px;

  & > summary {
    cursor: pointer;
    font-size: ${size.caption}px;
    font-weight: 700;
    color: ${color.textGray};
    margin-bottom: ${space.px6}px;
  }
`;

const ReasoningContent = styled.div`
  font-size: ${size.bodyXs}px;
  color: ${color.textDark};
  line-height: 1.6;
  padding: ${space.px8}px ${space.px12}px;
  border-left: 3px solid ${color.bgLightGray};
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 300px;
  overflow-y: auto;
`;

const SectionLabel = styled.div`
  font-size: ${size.mini}px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: ${color.textLightGray};
  margin-bottom: ${space.px6}px;
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
  vertical-align: top;
`;

const ValueTd = styled.td`
  padding: ${space.px4}px 0;
  text-align: right;
  color: ${color.textDark};
  font-size: ${size.bodyXs}px;
  word-break: break-word;
`;

const MissingTag = styled.span`
  color: ${color.redAccent};
  font-style: italic;
  font-size: ${size.caption}px;
`;

const Placeholder = styled.div`
  font-size: ${size.bodySm}px;
  color: ${color.textLightGray};
`;

const ErrorBox = styled.div`
  font-size: ${size.bodyXs}px;
  color: ${color.redAccent};
  padding: ${space.px8}px;
  border: 1px solid ${color.redAccent};
  margin-top: ${space.px8}px;
`;
