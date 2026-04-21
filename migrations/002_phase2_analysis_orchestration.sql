CREATE TABLE analysis_tasks (
    id UUID PRIMARY KEY,
    task_type TEXT NOT NULL CHECK (length(trim(task_type)) > 0),
    status TEXT NOT NULL CHECK (
        status IN (
            'pending',
            'running',
            'retry_waiting',
            'succeeded',
            'failed',
            'dead_letter',
            'cancelled'
        )
    ),
    instrument_id UUID NOT NULL REFERENCES instruments (id) ON DELETE CASCADE,
    user_id UUID,
    timeframe TEXT CHECK (timeframe IS NULL OR length(trim(timeframe)) > 0),
    bar_state TEXT NOT NULL CHECK (bar_state IN ('none', 'open', 'closed')),
    bar_open_time TIMESTAMPTZ,
    bar_close_time TIMESTAMPTZ,
    trading_date DATE,
    trigger_type TEXT NOT NULL CHECK (length(trim(trigger_type)) > 0),
    prompt_key TEXT NOT NULL CHECK (length(trim(prompt_key)) > 0),
    prompt_version TEXT NOT NULL CHECK (length(trim(prompt_version)) > 0),
    snapshot_id UUID NOT NULL UNIQUE,
    dedupe_key TEXT CHECK (dedupe_key IS NULL OR length(trim(dedupe_key)) > 0),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts INTEGER NOT NULL CHECK (max_attempts > 0),
    scheduled_at TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    last_error_code TEXT,
    last_error_message TEXT,
    CHECK (attempt_count <= max_attempts),
    CHECK (finished_at IS NULL OR started_at IS NULL OR finished_at >= started_at)
);

CREATE TABLE analysis_snapshots (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL UNIQUE REFERENCES analysis_tasks (id) ON DELETE CASCADE,
    input_json JSONB NOT NULL,
    input_hash TEXT NOT NULL,
    schema_version TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT analysis_snapshots_task_id_id_unique UNIQUE (task_id, id)
);

CREATE TABLE analysis_attempts (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES analysis_tasks (id) ON DELETE CASCADE,
    attempt_no INTEGER NOT NULL CHECK (attempt_no > 0),
    worker_id TEXT NOT NULL CHECK (length(trim(worker_id)) > 0),
    llm_provider TEXT NOT NULL CHECK (length(trim(llm_provider)) > 0),
    model TEXT NOT NULL CHECK (length(trim(model)) > 0),
    request_payload_json JSONB NOT NULL,
    raw_response_json JSONB,
    parsed_output_json JSONB,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed', 'cancelled')),
    error_type TEXT,
    error_message TEXT,
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ,
    CONSTRAINT analysis_attempts_task_attempt_unique UNIQUE (task_id, attempt_no),
    CHECK (finished_at IS NULL OR finished_at >= started_at)
);

CREATE TABLE analysis_results (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL UNIQUE REFERENCES analysis_tasks (id) ON DELETE CASCADE,
    task_type TEXT NOT NULL CHECK (length(trim(task_type)) > 0),
    instrument_id UUID NOT NULL REFERENCES instruments (id) ON DELETE CASCADE,
    user_id UUID,
    timeframe TEXT CHECK (timeframe IS NULL OR length(trim(timeframe)) > 0),
    bar_state TEXT NOT NULL CHECK (bar_state IN ('none', 'open', 'closed')),
    bar_open_time TIMESTAMPTZ,
    bar_close_time TIMESTAMPTZ,
    trading_date DATE,
    prompt_key TEXT NOT NULL CHECK (length(trim(prompt_key)) > 0),
    prompt_version TEXT NOT NULL CHECK (length(trim(prompt_version)) > 0),
    output_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE analysis_dead_letters (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL UNIQUE REFERENCES analysis_tasks (id) ON DELETE CASCADE,
    final_error_type TEXT NOT NULL CHECK (length(trim(final_error_type)) > 0),
    final_error_message TEXT NOT NULL CHECK (length(trim(final_error_message)) > 0),
    last_attempt_id UUID REFERENCES analysis_attempts (id) ON DELETE SET NULL,
    archived_snapshot_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE analysis_tasks
ADD CONSTRAINT analysis_tasks_snapshot_link_fkey
FOREIGN KEY (id, snapshot_id)
REFERENCES analysis_snapshots (task_id, id)
DEFERRABLE INITIALLY DEFERRED;
