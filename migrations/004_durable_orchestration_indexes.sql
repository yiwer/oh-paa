CREATE INDEX IF NOT EXISTS analysis_tasks_claim_idx
    ON analysis_tasks (status, scheduled_at, id);

CREATE UNIQUE INDEX IF NOT EXISTS analysis_tasks_dedupe_key_unique
    ON analysis_tasks (dedupe_key)
    WHERE dedupe_key IS NOT NULL;
