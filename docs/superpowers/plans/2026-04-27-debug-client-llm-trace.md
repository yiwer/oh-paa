# Debug Client: LLM Trace View + Cross-View Navigation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the LLM Trace view (2 pages: Trigger List + Task Detail) and wire cross-view navigation between all three views.

**Architecture:** Page 1 shows instrument-scoped trigger list with expandable task chains. Page 2 shows full LLM interaction for a single task with timeline visualization. Cross-view links connect Pipeline → K-Line → LLM Trace bidirectionally.

**Tech Stack:** React 19 · TypeScript · styled-components 6 · TanStack React Query 5 · React Router v7

**Sub-project scope:** Plan 3 of 3. Builds on Plans 1-2.

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `web/src/api/hooks/useLlmTrace.ts` | React Query hooks for tasks, attempts, results |
| Create | `web/src/pages/LlmTracePage.tsx` | Page 1: Trigger List |
| Create | `web/src/pages/llm-trace/TriggerRow.tsx` | Collapsible trigger with task chain |
| Create | `web/src/pages/llm-trace/TaskChainNode.tsx` | Single task node in vertical timeline |
| Create | `web/src/pages/llm-trace/PipelineStatusDots.tsx` | Inline ● → ● → ● status indicator |
| Create | `web/src/pages/TaskDetailPage.tsx` | Page 2: Full task detail with LLM interaction |
| Create | `web/src/pages/task-detail/TimelineCard.tsx` | Card with left timeline node connector |
| Create | `web/src/pages/task-detail/PromptInputCard.tsx` | Card 1: structured prompt input |
| Create | `web/src/pages/task-detail/LlmResponseCard.tsx` | Card 2: response + reasoning chain |
| Create | `web/src/pages/task-detail/ValidationCard.tsx` | Card 3: schema validation result |
| Create | `web/src/pages/task-detail/ResultCard.tsx` | Card 4: analysis result or dead letter |
| Modify | `web/src/App.tsx` | Add routes for LlmTracePage and TaskDetailPage |

---

## Task 1: API Hooks for LLM Trace

**Create:** `web/src/api/hooks/useLlmTrace.ts`

React Query hooks wrapping existing analysis REST endpoints:

```typescript
import { useQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import type { AnalysisTask, AnalysisAttempt, AnalysisResult, AnalysisDeadLetter } from '@/api/types';

export function useTasks(instrumentId: string) {
  return useQuery({
    queryKey: ['tasks', instrumentId],
    queryFn: () => api<{ rows: AnalysisTask[] }>(
      `/analysis/tasks?instrument_id=${instrumentId}`
    ).then(r => r.rows),
    enabled: !!instrumentId,
    staleTime: 10_000,
  });
}

export function useTask(taskId: string) {
  return useQuery({
    queryKey: ['task', taskId],
    queryFn: () => api<AnalysisTask>(`/analysis/tasks/${taskId}`),
    enabled: !!taskId,
  });
}

export function useAttempts(taskId: string) {
  return useQuery({
    queryKey: ['attempts', taskId],
    queryFn: () => api<{ rows: AnalysisAttempt[] }>(
      `/analysis/tasks/${taskId}/attempts`
    ).then(r => r.rows),
    enabled: !!taskId,
  });
}

export function useResult(taskId: string) {
  return useQuery({
    queryKey: ['result', taskId],
    queryFn: () => api<AnalysisResult>(`/analysis/results/${taskId}`),
    enabled: !!taskId,
  });
}

export function useDeadLetter(taskId: string) {
  return useQuery({
    queryKey: ['dead-letter', taskId],
    queryFn: () => api<AnalysisDeadLetter>(`/analysis/dead-letters/${taskId}`),
    enabled: !!taskId,
  });
}
```

Commit: `feat(web): add LLM trace API hooks`

---

## Task 2: PipelineStatusDots + TriggerRow + TaskChainNode

**Create:**
- `web/src/pages/llm-trace/PipelineStatusDots.tsx`
- `web/src/pages/llm-trace/TriggerRow.tsx`
- `web/src/pages/llm-trace/TaskChainNode.tsx`

### PipelineStatusDots

Inline `● → ● → ●` indicator. Each dot colored by task status. Props: tasks for a trigger grouped by task_type.

### TriggerRow

Collapsible row for one trigger (instrument + timeframe + bar_close_time). Collapsed shows trigger info + PipelineStatusDots. Expanded shows vertical TaskChainNode list.

### TaskChainNode

A single task in the vertical timeline. Left: colored node (✓/✗/☠) + vertical line. Right: task card with type, status badge, provider, latency, output summary. Click "→ detail" navigates to TaskDetailPage.

Commit: `feat(web): add LLM trace trigger row and task chain components`

---

## Task 3: LlmTracePage (Page 1)

**Create:** `web/src/pages/LlmTracePage.tsx`

Layout:
- Top: InstrumentSwitcher (left) + market toggle + task type/status filters
- MetricStrip: Triggers | Passed | In Progress | Errors | Avg Latency
- Trigger list: grouped by instrument (from switcher), sorted by time desc
- Each trigger is a TriggerRow

Group tasks into triggers: same instrument_id + timeframe + bar_close_time = one trigger.

Commit: `feat(web): implement LLM Trace Page 1 (Trigger List)`

---

## Task 4: Task Detail Page Components

**Create:**
- `web/src/pages/task-detail/TimelineCard.tsx`
- `web/src/pages/task-detail/PromptInputCard.tsx`
- `web/src/pages/task-detail/LlmResponseCard.tsx`
- `web/src/pages/task-detail/ValidationCard.tsx`
- `web/src/pages/task-detail/ResultCard.tsx`

### TimelineCard

Wrapper component that renders a timeline node on the left + card content on the right.
Props: `icon` (→/←/✓/✗/◎/☠), `color` (dark/teal/red/yellow), `lineColor`, `isLast`, `children`.

### PromptInputCard

Three-column layout: Task Context (key-value) | K-Line Input (table) | PA State (key-value).
All data parsed from the task's snapshot `input_json`. Bottom: "View full prompt text →".

### LlmResponseCard

Meta row: Provider / Model / Latency / HTTP Status.
Collapsible reasoning chain (details/summary).
Output fields: structured key-value table from `parsed_output_json`. Missing fields marked with ⚠.

### ValidationCard

Success: "Schema Pass" in teal.
Failure: Error Type / Message / Retryable / Retry Decision table.

### ResultCard

Success: highlighted summary (pattern + bar reading) + meta (Task ID, Schema Version, timestamps).
Dead letter: gray dashed border, Final Error / Attempts / Archived At.

Commit: `feat(web): add task detail card components`

---

## Task 5: TaskDetailPage (Page 2) + Router

**Create:** `web/src/pages/TaskDetailPage.tsx`
**Modify:** `web/src/App.tsx`

### TaskDetailPage

Route: `/llm-trace/:taskId`

Layout:
- Breadcrumb: ← {instrument} Triggers / {timeframe} · {bar_time} / {task_type}
- Header: task type + status badge + provider/latency
- Attempt tabs (if multiple attempts): tab per attempt with label
- Four timeline cards (left vertical line connecting them):
  1. PromptInputCard (→ node)
  2. LlmResponseCard (← node)
  3. ValidationCard (✓/✗ node)
  4. ResultCard (◎/☠ node)

Data: `useTask(taskId)`, `useAttempts(taskId)`, `useResult(taskId)`, `useDeadLetter(taskId)`.

### Router

Add routes:
```tsx
<Route path="/llm-trace" element={<LlmTracePage />} />
<Route path="/llm-trace/:taskId" element={<TaskDetailPage />} />
```

Commit: `feat(web): implement Task Detail page with timeline visualization`

---

## Task 6: Cross-View Navigation

**Modify multiple files** to add navigation links:

1. Pipeline InstrumentCard: click symbol → `/kline?instrument={id}`
2. KLine DataInspector: "Open in LLM Trace →" → `/llm-trace/:taskId` (placeholder, need task lookup)
3. TaskDetailPage header: "View in K-Line →" → `/kline?instrument={id}&timeframe={tf}`
4. TaskDetailPage breadcrumb: "← Triggers" → `/llm-trace`

Use `useNavigate()` and `useSearchParams()` from react-router-dom.

For KLinePage: read `instrument` and `timeframe` from search params as initial state.

Commit: `feat(web): add cross-view navigation links`
