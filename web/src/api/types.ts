/* ------------------------------------------------------------------ */
/*  Domain types — mirrors Rust structs serialised by the API          */
/* ------------------------------------------------------------------ */

export interface Market {
  id: string;
  code: string;
  name: string;
  timezone: string;
}

export interface Instrument {
  id: string;
  market_id: string;
  symbol: string;
  name: string;
  instrument_type: string;
}

export interface CanonicalKline {
  instrument_id: string;
  timeframe: string;
  open_time: string;
  close_time: string;
  open: string;
  high: string;
  low: string;
  close: string;
  volume: string | null;
  source_provider: string;
}

export interface AggregatedKline {
  instrument_id: string;
  source_timeframe: string;
  timeframe: string;
  open_time: string;
  close_time: string;
  open: string;
  high: string;
  low: string;
  close: string;
  volume: string | null;
  complete: boolean;
  child_bar_count: number;
  expected_child_bar_count: number;
  source_provider: string;
}

export interface AnalysisTask {
  id: string;
  snapshot_id: string;
  task_type: string;
  status: string;
  instrument_id: string;
  timeframe: string | null;
  bar_state: string;
  bar_open_time: string | null;
  bar_close_time: string | null;
  prompt_key: string;
  prompt_version: string;
  attempt_count: number;
  max_attempts: number;
  started_at: string | null;
  finished_at: string | null;
  last_error_code: string | null;
  last_error_message: string | null;
}

export interface AnalysisAttempt {
  id: string;
  task_id: string;
  attempt_number: number;
  worker_id: string;
  llm_provider: string;
  model: string;
  request_payload_json: unknown;
  raw_response_json: unknown | null;
  parsed_output_json: unknown | null;
  error_type: string | null;
  error_message: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface AnalysisResult {
  id: string;
  task_id: string;
  output_json: unknown;
  created_at: string;
}

export interface AnalysisDeadLetter {
  id: string;
  task_id: string;
  archived_snapshot_json: unknown;
  final_error_type: string;
  final_error_message: string;
  last_attempt_id: string | null;
  created_at: string;
}

export type DebugEventType =
  | 'kline_ingested'
  | 'provider_fallback'
  | 'normalization_result'
  | 'task_status_changed'
  | 'attempt_completed'
  | 'open_bar_update';

export interface DebugEvent {
  type: DebugEventType;
  [key: string]: unknown;
}

export interface SessionProfile {
  market_code: string;
  market_timezone: string;
  session_kind: string;
}

export interface BarReading {
  instrument_id: string;
  timeframe: string;
  bar_close_time: string;
  bar_reading_label: string;
  bar_reading_color: 'red' | 'green' | 'gray' | 'yellow';
  bar_summary: string;
  pattern: string;
  structure: string;
  bias: string;
  source: string;
}

export interface KeyLevel {
  price: string;
  label: string;
  type: 'support' | 'resistance' | 'target';
}

export interface OpenBar {
  instrument_id: string;
  timeframe: string;
  open_time: string;
  open: string;
  high: string;
  low: string;
  close: string;
}
