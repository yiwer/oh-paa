# Dashscope → Deepseek-v4 Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove dashscope from both example configs and route the `pa_state_extract_fast` profile to `deepseek-v4-flash`; align the replay-quality template with the main config's deepseek-v4 profiles.

**Architecture:** Pure config-layer change. The orchestrator's `OpenAiCompatibleClient` already speaks OpenAI-compatible chat completions to any provider URL — no Rust code changes are needed. The deepseek-only `thinking` extension in `crates/pa-orchestrator/src/openai_client.rs` and all `"dashscope"` string fixtures across tests are intentionally left untouched (per spec decisions Q2 and Q3).

**Tech Stack:** TOML configuration files; `cargo test` for regression verification.

**Spec:** `docs/superpowers/specs/2026-04-30-dashscope-to-deepseek-v4-migration-design.md`

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `config.example.toml` | Modify | Main runtime example. Drop dashscope provider block; switch `pa_state_extract_fast` to deepseek-v4-flash. |
| `config.live-replay-quality.example.toml` | Modify | Replay-quality runtime example. Drop dashscope provider block; align all three execution profiles to the main config's deepseek-v4 settings. |

No new files. No Rust source touched. No new tests (regression-only verification via `cargo test --workspace`).

---

## Task 1: Migrate `config.example.toml`

**Files:**
- Modify: `config.example.toml`

**Goal:** Delete the `[llm.providers.dashscope]` block (lines 13–16) and change `pa_state_extract_fast` to use `deepseek` / `deepseek-v4-flash`.

- [ ] **Step 1: Capture baseline test result**

Run: `cargo test -p pa-core config --no-fail-fast`

Expected: PASS (this baseline confirms the inline TOML fixture inside `crates/pa-core/tests/config.rs` is not coupled to `config.example.toml` — the example file is documentation, not loaded by tests).

If this fails, stop and investigate before continuing — the migration assumes a green baseline.

- [ ] **Step 2: Delete the `[llm.providers.dashscope]` block**

Edit `config.example.toml`. Locate this block at lines 13–16 (and the trailing blank line on line 17):

```toml
[llm.providers.dashscope]
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
api_key = "replace-with-real-key"
openai_api_style = "chat_completions"

```

Delete all five lines (the four config lines plus the trailing blank line). The file should now have `[llm.providers.deepseek]` block (lines 8–11) followed directly by `[llm.execution_profiles.pa_state_extract_fast]`.

- [ ] **Step 3: Change `pa_state_extract_fast` to deepseek-v4-flash**

In the same file, find:

```toml
[llm.execution_profiles.pa_state_extract_fast]
provider = "dashscope"
model = "qwen-plus"
```

Replace with:

```toml
[llm.execution_profiles.pa_state_extract_fast]
provider = "deepseek"
model = "deepseek-v4-flash"
```

Leave `max_tokens = 12000`, `max_retries = 2`, `per_call_timeout_secs = 180`, `retry_initial_backoff_ms = 1000`, `supports_json_schema = false`, `supports_reasoning = false` unchanged.

- [ ] **Step 4: Verify the file parses as valid TOML and visually matches expected end-state**

Run: `cargo run --quiet --bin pa-app -- --help` if a CLI exists that loads the config; otherwise skip to Step 5.

If unsure whether such a CLI exists, run:

`cargo build --workspace`

Expected: PASS. (TOML parsing happens at runtime, not build time, so this just confirms nothing else regressed. The strict TOML validation happens in Step 5.)

Then visually inspect the file. The full expected content is:

```toml
database_url = "postgres://postgres:pgsql@localhost:5432/oh_paa"
server_addr = "127.0.0.1:3000"
bootstrap_local_test_instruments = false
eastmoney_base_url = "https://push2his.eastmoney.com/"
twelvedata_base_url = "https://api.twelvedata.com/"
twelvedata_api_key = "replace-with-real-key"

[llm.providers.deepseek]
base_url = "https://api.deepseek.com"
api_key = "replace-with-real-key"
openai_api_style = "chat_completions"

[llm.execution_profiles.pa_state_extract_fast]
provider = "deepseek"
model = "deepseek-v4-flash"
max_tokens = 12000
max_retries = 2
per_call_timeout_secs = 180
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.execution_profiles.shared_bar_reasoner]
provider = "deepseek"
model = "deepseek-v4-pro"
max_tokens = 32768
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = true

[llm.execution_profiles.user_position_reasoner]
provider = "deepseek"
model = "deepseek-v4-flash"
max_tokens = 16384
max_retries = 2
per_call_timeout_secs = 300
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.step_bindings.shared_pa_state_bar_v1]
execution_profile = "pa_state_extract_fast"

[llm.step_bindings.shared_bar_analysis_v2]
execution_profile = "shared_bar_reasoner"

[llm.step_bindings.shared_daily_context_v2]
execution_profile = "shared_bar_reasoner"

[llm.step_bindings.user_position_advice_v2]
execution_profile = "user_position_reasoner"
```

- [ ] **Step 5: Validate TOML parses + cross-references resolve**

Run: `cargo test -p pa-core config --no-fail-fast`

Expected: PASS (same as baseline). The inline TOML fixture in `tests/config.rs` is not coupled to the example, so this should be unchanged.

If you can also locate a test that loads `config.example.toml` directly, run it. Otherwise, run a one-liner TOML parse check:

`cargo run --quiet -p pa-app -- --config config.example.toml --help 2>&1 | head -20`

Expected: either a help banner or a clear error unrelated to TOML syntax (such as a missing DB connection, which is fine — we only care that the file parses).

If you see a TOML parse error or "execution_profile not found" / "unknown provider" error, the file is malformed — re-check Step 2 and Step 3.

- [ ] **Step 6: Commit**

```bash
git add config.example.toml
git commit -m "chore(config): migrate pa_state_extract_fast off dashscope to deepseek-v4-flash"
```

---

## Task 2: Migrate `config.live-replay-quality.example.toml`

**Files:**
- Modify: `config.live-replay-quality.example.toml`

**Goal:** Delete the `[llm.providers.dashscope]` block (lines 17–20) and align all three execution profiles to the main config's deepseek-v4 settings (per spec decision Q4=B).

- [ ] **Step 1: Delete the `[llm.providers.dashscope]` block**

Edit `config.live-replay-quality.example.toml`. Locate this block at lines 17–20 (and the trailing blank line on line 21):

```toml
[llm.providers.dashscope]
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
api_key = "replace-with-real-key"
openai_api_style = "chat_completions"

```

Delete all five lines.

- [ ] **Step 2: Replace all three execution profiles to match the main config**

In the same file, find this block (current lines 22–50):

```toml
[llm.execution_profiles.pa_state_extract_fast]
provider = "dashscope"
model = "qwen3.6-plus"
max_tokens = 4096
max_retries = 2
per_call_timeout_secs = 180
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.execution_profiles.shared_bar_reasoner]
provider = "dashscope"
model = "qwen3-max"
max_tokens = 8192
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.execution_profiles.user_position_reasoner]
provider = "deepseek"
model = "deepseek-v4-flash"
max_tokens = 4096
max_retries = 2
per_call_timeout_secs = 300
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false
```

Replace with:

```toml
[llm.execution_profiles.pa_state_extract_fast]
provider = "deepseek"
model = "deepseek-v4-flash"
max_tokens = 12000
max_retries = 2
per_call_timeout_secs = 180
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.execution_profiles.shared_bar_reasoner]
provider = "deepseek"
model = "deepseek-v4-pro"
max_tokens = 32768
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = true

[llm.execution_profiles.user_position_reasoner]
provider = "deepseek"
model = "deepseek-v4-flash"
max_tokens = 16384
max_retries = 2
per_call_timeout_secs = 300
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false
```

Three changes vs. the prior content:
- `pa_state_extract_fast`: dashscope/qwen3.6-plus @ 4096 tokens → deepseek-v4-flash @ 12000.
- `shared_bar_reasoner`: dashscope/qwen3-max @ 8192 / reasoning off → deepseek-v4-pro @ 32768 / reasoning **on**.
- `user_position_reasoner`: max_tokens 4096 → 16384 (provider/model already deepseek-v4-flash).

The header comment (lines 1–4) and `[llm.step_bindings.*]` blocks remain unchanged.

- [ ] **Step 3: Visually verify expected end-state**

The full expected content is:

```toml
# Live replay quality template.
# For replay quality work, copy this file to a local uncommitted config and pass it via:
#   --config <local-live-replay-quality.toml>
# Do not use config.example.toml for funnel runs.

database_url = "postgres://postgres:pgsql@localhost:5432/oh_paa"
server_addr = "127.0.0.1:3000"
eastmoney_base_url = "https://push2his.eastmoney.com/"
twelvedata_base_url = "https://api.twelvedata.com/"
twelvedata_api_key = "replace-with-real-key"

[llm.providers.deepseek]
base_url = "https://api.deepseek.com"
api_key = "replace-with-real-key"
openai_api_style = "chat_completions"

[llm.execution_profiles.pa_state_extract_fast]
provider = "deepseek"
model = "deepseek-v4-flash"
max_tokens = 12000
max_retries = 2
per_call_timeout_secs = 180
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.execution_profiles.shared_bar_reasoner]
provider = "deepseek"
model = "deepseek-v4-pro"
max_tokens = 32768
max_retries = 2
per_call_timeout_secs = 600
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = true

[llm.execution_profiles.user_position_reasoner]
provider = "deepseek"
model = "deepseek-v4-flash"
max_tokens = 16384
max_retries = 2
per_call_timeout_secs = 300
retry_initial_backoff_ms = 1000
supports_json_schema = false
supports_reasoning = false

[llm.step_bindings.shared_pa_state_bar_v1]
execution_profile = "pa_state_extract_fast"

[llm.step_bindings.shared_bar_analysis_v2]
execution_profile = "shared_bar_reasoner"

[llm.step_bindings.shared_daily_context_v2]
execution_profile = "shared_bar_reasoner"

[llm.step_bindings.user_position_advice_v2]
execution_profile = "user_position_reasoner"
```

The execution-profile section between the deepseek provider block and the step bindings should match the main `config.example.toml` byte-for-byte (modulo the absent `bootstrap_local_test_instruments = false` line, which only lives in the main config).

- [ ] **Step 4: Validate TOML parses**

Run: `cargo run --quiet -p pa-app -- --config config.live-replay-quality.example.toml --help 2>&1 | head -20`

Expected: help banner or an error unrelated to TOML syntax. If you see a TOML parse error or unknown-provider error, re-check Step 1 and Step 2.

If the binary doesn't accept `--help` with `--config`, an alternate quick check:

`cargo test -p pa-core config --no-fail-fast`

Expected: PASS (same baseline as Task 1 Step 1 — confirms no regression).

- [ ] **Step 5: Commit**

```bash
git add config.live-replay-quality.example.toml
git commit -m "chore(config): align replay-quality template with deepseek-v4 main profiles"
```

---

## Task 3: Workspace regression check

**Files:** None modified.

**Goal:** Confirm the workspace still builds and tests still pass after the two config changes.

- [ ] **Step 1: Run the full workspace test suite**

Run: `cargo test --workspace --no-fail-fast`

Expected: PASS (same green status as before the migration). All `"dashscope"` string fixtures across tests remain valid because they are opaque envelope labels — they are NOT validated against any registered provider name in those tests. If any test fails referring to `"dashscope"`, stop and investigate; that would mean my Q3=A scope assumption was wrong and the spec needs revision before proceeding.

- [ ] **Step 2: Confirm no further commits needed**

Run: `git status`

Expected: working tree clean (the two commits from Task 1 and Task 2 are the only changes).

If clean, the migration is complete. No commit needed for this task.

---

## Self-Review

**Spec coverage check:**

| Spec section | Implementing task |
|---|---|
| Decision Q1: pa_state_extract_fast → deepseek-v4-flash | Task 1 Step 3 |
| Decision Q2: openai_client.rs untouched | (no task — explicit non-goal verified by Task 3) |
| Decision Q3: delete dashscope provider block, leave string fixtures | Task 1 Step 2 + Task 2 Step 1 (delete); Task 3 Step 1 (verifies fixtures still pass) |
| Decision Q4: replay template aligns with main config | Task 2 Step 2 |
| Verification: cargo test -p pa-core | Task 1 Step 1 (baseline) + Step 5 (post-change) |
| Verification: cargo test --workspace | Task 3 Step 1 |
| Verification: manual smoke (deepseek-v4-flash routing) | Out of plan scope — requires real API key. Documented in spec as a manual post-deployment check. |

**Placeholder scan:** No TBD/TODO/vague items. Every step has either a concrete TOML block or an exact command with expected output.

**Type/name consistency:** Profile names (`pa_state_extract_fast`, `shared_bar_reasoner`, `user_position_reasoner`) and provider/model names (`deepseek`, `deepseek-v4-flash`, `deepseek-v4-pro`) are spelled identically in every task and match the spec table. No method-signature concerns since no code is being written.
