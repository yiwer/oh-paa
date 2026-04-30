# Dashscope → Deepseek-v4 Migration (Config-Layer Only)

**Date:** 2026-04-30
**Status:** Design — pending user approval

## Background

The orchestrator already speaks OpenAI-compatible chat completions to all configured providers via `crates/pa-orchestrator/src/openai_client.rs` (`OpenAiCompatibleClient`). Two providers are wired up in the example configs: `deepseek` (`https://api.deepseek.com`) and `dashscope` (`https://dashscope.aliyuncs.com/compatible-mode/v1`, Alibaba's OpenAI-compatible mode).

Three execution profiles exist. As of the current `config.example.toml`:

| Profile | Provider | Model | Bound steps |
|---|---|---|---|
| `pa_state_extract_fast` | **dashscope** | qwen-plus | `shared_pa_state_bar_v1` |
| `shared_bar_reasoner` | deepseek | deepseek-v4-pro | `shared_bar_analysis_v2`, `shared_daily_context_v2` |
| `user_position_reasoner` | deepseek | deepseek-v4-flash | `user_position_advice_v2` |

Only `pa_state_extract_fast` still routes to dashscope/qwen-plus; the other two profiles already use deepseek-v4 models.

The replay-quality template `config.live-replay-quality.example.toml` deliberately diverges from the main config — it uses dashscope/qwen3.6-plus and dashscope/qwen3-max with smaller `max_tokens` and reasoning disabled, as a low-cost evaluation harness.

## Goal

Remove dashscope from the example configs so the project's example LLM surface is exclusively deepseek-v4 (-flash / -pro), reachable via a single OpenAI-compatible base URL.

## Decisions

| # | Question | Decision |
|---|---|---|
| Q1 | Which model replaces qwen-plus for `pa_state_extract_fast`? | **deepseek-v4-flash** — the profile's role is fast, low-latency state extraction; flash matches that envelope. |
| Q2 | Drop the deepseek-only `thinking` / `reasoning_effort` branch in `openai_client.rs`? | **No** — leave `crates/pa-orchestrator/src/openai_client.rs` untouched. The deepseek `thinking: { type: enabled/disabled }` extension stays. Out of scope for this migration. |
| Q3 | How thoroughly to scrub dashscope from the repo? | **Delete the `[llm.providers.dashscope]` blocks from the two example configs.** Test/source string fixtures referencing `"dashscope"` are opaque envelope labels and remain untouched **except** for two regression assertions that read `config.example.toml` directly and must track the new content (see "Q3 correction" below). |
| Q4 | How to handle the replay-quality template, which had distinct low-cost parameters? | **Align with main config** — replace dashscope/qwen3-max with deepseek-v4-pro at the same params as the main config. The template's "low-cost / reasoning disabled" stance is intentionally dropped. |

## Q3 correction (added during execution)

The original Q3 framing assumed every `*.rs` reference to `"dashscope"` was an opaque envelope label. Implementation discovered two assertions that are real regression checks coupled to `config.example.toml` content:

1. **`crates/pa-core/tests/config.rs:272`** — inside `load_from_path_parses_config_example_toml`:
   ```rust
   assert!(config.llm.providers.contains_key("dashscope"));
   ```
   This loads the actual `config.example.toml` from disk and asserts the dashscope provider key is present. After deletion of that block, this assertion fails.

2. **`crates/pa-app/tests/replay.rs:73-74`** — inside `replay_runner_records_variant_step_outputs_and_scores`:
   ```rust
   assert_eq!(first.llm_provider, "dashscope");
   assert_eq!(first.model, "qwen-plus");
   ```
   The fixture replay path calls `load_example_config()` (`crates/pa-app/src/replay.rs:238`), which reads `config.example.toml` and injects the profile's provider/model into each step-run envelope. After the migration, the first step's envelope reads `llm_provider="deepseek"` and `model="deepseek-v4-flash"`.

Both assertions must be updated to match the new example-config content:
- `config.rs:272`: drop the `dashscope` assertion (or replace with a `deepseek` provider check; the test already asserts `deepseek` on line 271).
- `replay.rs:73-74`: `"dashscope"` → `"deepseek"`, `"qwen-plus"` → `"deepseek-v4-flash"`.

All other `"dashscope"` string references across tests/sources remain untouched, consistent with the original Q3=A intent for inert envelope labels.

### Out-of-scope test failure (documented)

`crates/pa-core/tests/config.rs:343-352` (`load_from_path_parses_repo_config_toml_with_local_bootstrap_enabled`) loads the gitignored local `config.toml` and asserts `bootstrap_local_test_instruments == true`. This test is environment-dependent (reads the user's local config) and is failing pre-migration as well. Not in this migration's scope.

## Scope

### In scope (2 files + 2 test files post-correction)

- `config.example.toml`
- `config.live-replay-quality.example.toml`
- `crates/pa-core/tests/config.rs` (Q3 correction — 1 assertion)
- `crates/pa-app/tests/replay.rs` (Q3 correction — 2 assertions)

### Out of scope

- `crates/pa-orchestrator/src/openai_client.rs` — including its deepseek `thinking` branch (Q2).
- Inert `"dashscope"` envelope-label fixtures (Q3, original intent — these remain untouched):
  - `crates/pa-orchestrator/tests/{executor.rs,models.rs}`
  - `crates/pa-orchestrator/src/openai_client.rs` (test module)
  - `crates/pa-core/tests/config.rs` (inline TOML fixture string `"dashscope"`, independent of the example files — distinct from the line-272 assertion which IS updated per Q3 correction)
  - `crates/pa-app/src/{lib.rs,replay.rs,replay_probe.rs}`
  - `crates/pa-app/tests/{replay_config.rs,probe.rs,live_replay.rs}` (note: `replay.rs` IS updated per Q3 correction)
- `config.toml` — gitignored local runtime config; the user updates it locally to mirror the example.
- The pre-existing failure of `load_from_path_parses_repo_config_toml_with_local_bootstrap_enabled` (see Q3 correction section).

## Changes

### `config.example.toml`

1. Delete the `[llm.providers.dashscope]` block (current lines 13–16).
2. Update `[llm.execution_profiles.pa_state_extract_fast]`:
   - `provider`: `"dashscope"` → `"deepseek"`
   - `model`: `"qwen-plus"` → `"deepseek-v4-flash"`
   - All other fields unchanged: `max_tokens=12000`, `max_retries=2`, `per_call_timeout_secs=180`, `retry_initial_backoff_ms=1000`, `supports_json_schema=false`, `supports_reasoning=false`.

Resulting profile:

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
```

### `config.live-replay-quality.example.toml`

1. Delete the `[llm.providers.dashscope]` block (current lines 17–20).
2. Replace all three execution profiles to match the main config exactly:

| Profile | provider | model | max_tokens | max_retries | per_call_timeout_secs | retry_initial_backoff_ms | supports_json_schema | supports_reasoning |
|---|---|---|---|---|---|---|---|---|
| `pa_state_extract_fast` | deepseek | deepseek-v4-flash | 12000 | 2 | 180 | 1000 | false | false |
| `shared_bar_reasoner` | deepseek | deepseek-v4-pro | 32768 | 2 | 600 | 1000 | false | **true** |
| `user_position_reasoner` | deepseek | deepseek-v4-flash | 16384 | 2 | 300 | 1000 | false | false |

Notable shifts vs. the prior replay template:

- `shared_bar_reasoner`: dashscope/qwen3-max @ 8192 tokens / reasoning off → deepseek-v4-pro @ 32768 tokens / reasoning on.
- `user_position_reasoner`: max_tokens 4096 → 16384.
- `pa_state_extract_fast`: dashscope/qwen3.6-plus @ 4096 tokens → deepseek-v4-flash @ 12000 tokens.

Header comment ("Live replay quality template…") is retained — the file's role becomes "an isolated replay-quality config that mirrors main rather than diverges from it."

## Verification

1. `cargo test -p pa-core` — `crates/pa-core/tests/config.rs` carries its own inline TOML (not loaded from the example files), so it must continue to pass unchanged. This is a direct consequence of Q3=A.
2. `cargo test --workspace` — should remain green; no Rust source touched.
3. Manual smoke: run a single `shared_pa_state_bar_v1` step using the updated main config. Expected envelope fields: `llm_provider="deepseek"`, `model="deepseek-v4-flash"`, request hits `https://api.deepseek.com/chat/completions`.

## Non-goals

- Refactoring or simplifying `openai_client.rs` (including the `thinking` deepseek extension).
- Renaming `"dashscope"` string fixtures across tests.
- Touching the user's local `config.toml`.
- Changing prompt registry, step bindings, or any orchestrator behavior.

## Risk & rollback

Risk surface is small: configuration only. Rollback is `git revert` of the single config commit. The deepseek API key in `config.toml` must be valid for both v4-flash and v4-pro; if v4-flash is not yet enabled on the user's account, `pa_state_extract_fast` calls will fail at the provider with a clear error — this is detectable at first run and resolved by enabling the model on the account, not by code change.
