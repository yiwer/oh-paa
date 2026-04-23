# 2026-04-23 Live Replay Findings

## Candidate qwenplus-dsreasoner-dschat

- Config:
  - `shared_pa_state_bar_v1 -> dashscope/qwen-plus`
  - `shared_bar_analysis_v2 -> deepseek/deepseek-reasoner`
  - `shared_daily_context_v2 -> deepseek/deepseek-reasoner`
  - `user_position_advice_v2 -> deepseek/deepseek-chat`
- Stage 3 result:
  - source report: `G:\Rust\oh-paa\docs\superpowers\archives\2026-04-23-live-crypto-15m-report.json`
  - `schema_hit_rate = 1.0`
  - `cross_step_consistency_rate = 1.0`
  - `decision_tree_completeness = 1.0`
  - `key_level_completeness = 1.0`
  - `signal_bar_completeness = 1.0`
  - `bull_bear_dual_path_completeness = 1.0`
  - `step_runs = 20`
  - first failing step: none
- Status:
  - first full `5-sample` success observed
  - second confirmation run failed before report generation
  - failure log: `G:\Rust\oh-paa\.worktrees\live-replay-quality-opt\docs\superpowers\archives\2026-04-23-candidate-qwenplus-dsreasoner-dschat-confirmation-run.log`
  - first failing step in confirmation run: warmup `shared_bar_analysis:v2` for sample `crypto-btcusd-2026-04-18t08-30z`
  - failure category: transport
  - provider error: `error decoding response body` with nested `operation timed out`

## Candidate step1-dsv32-step23-qwen36plus

- Config:
  - `shared_pa_state_bar_v1 -> dashscope/deepseek-v3.2`
  - `shared_bar_analysis_v2 -> dashscope/qwen3.6-plus`
  - `shared_daily_context_v2 -> dashscope/qwen3.6-plus`
  - `user_position_advice_v2 -> deepseek/deepseek-v3.2`
- Stage 1 result:
  - repeated probe calls returned schema-valid JSON for step 1
  - output still showed semantic drift and unsupported inference risk
- Stage 2 result:
  - source log: `G:\Rust\oh-paa\.worktrees\live-replay-quality-opt\docs\superpowers\archives\2026-04-23-candidate-step1-dsv32-step23-qwen36plus-stage2-run.log`
  - fail
  - first failing step: warmup `shared_bar_analysis:v2`
  - failure category: transport
  - provider error: `403 Forbidden` on DashScope chat completions
- Status:
  - rejected at Stage 2
  - next variable was to remove DashScope from step 2/3

## Candidate step1-dsv32-step23-dsreasoner

- Config:
  - `shared_pa_state_bar_v1 -> dashscope/deepseek-v3.2`
  - `shared_bar_analysis_v2 -> deepseek/deepseek-reasoner`
  - `shared_daily_context_v2 -> deepseek/deepseek-reasoner`
  - `user_position_advice_v2 -> deepseek/deepseek-v3.2`
- Stage 1 result:
  - three repeated probe calls were schema-valid
  - output remained noisy and occasionally invented context beyond the supplied input
- Stage 2 result:
  - source report: `G:\Rust\oh-paa\.worktrees\live-replay-quality-opt\docs\superpowers\archives\2026-04-23-candidate-step1-dsv32-step23-dsreasoner-stage2-report.json`
  - `schema_hit_rate = 0.75`
  - `cross_step_consistency_rate = 1.0`
  - `decision_tree_completeness = 1.0`
  - `key_level_completeness = 1.0`
  - `signal_bar_completeness = 1.0`
  - `bull_bear_dual_path_completeness = 1.0`
  - first failing step: `user_position_advice:v2`
  - failure category: transport
  - provider error: `400 Bad Request` on DeepSeek chat completions
- Status:
  - rejected at Stage 2
  - step 1 is not yet strong enough to displace the winning baseline even before cost or latency are considered

## Candidate step2-qwen-plus-hardened

- Config:
  - `shared_pa_state_bar_v1 -> dashscope/qwen-plus`
  - `shared_bar_analysis_v2 -> dashscope/qwen-plus`
  - `shared_daily_context_v2 -> deepseek/deepseek-reasoner`
  - `user_position_advice_v2 -> deepseek/deepseek-chat`
- Prompt/schema hardening:
  - tightened `shared_bar_analysis_v2` top-level schema with `additionalProperties = false`
  - required `key_levels` to keep the named object slots `immediate_support`, `immediate_resistance`, `next_support_below`, `next_resistance_above`
  - added prompt instructions that `key_levels` children must stay single objects and `follow_through_checkpoints` must not collapse into an array
- Probe result:
  - repeated `shared_bar_analysis:v2` probe calls on the real `08:30Z` input were `3/3 schema_valid`
  - observed probe latency for `shared_bar_analysis:v2` with `dashscope/qwen-plus`: about `58.9s`
  - observed output shape stayed on the hardened object contract for both `key_levels` and `follow_through_checkpoints`
- Stage 2 result:
  - source report: `G:\Rust\oh-paa\.worktrees\live-replay-quality-opt\docs\superpowers\archives\2026-04-23-candidate-step2-dashscope-qwen-plus-hardened-stage2-report.json`
  - `schema_hit_rate = 1.0`
  - `cross_step_consistency_rate = 1.0`
  - `decision_tree_completeness = 1.0`
  - `key_level_completeness = 1.0`
  - `signal_bar_completeness = 1.0`
  - `bull_bear_dual_path_completeness = 1.0`
  - `avg_latency_ms = 69188.25`
  - first failing step: none
  - target step latencies:
    - `shared_pa_state_bar_v1`: `48071ms`
    - `shared_bar_analysis_v2`: `64073ms`
    - `shared_daily_context_v2`: `121537ms`
    - `user_position_advice_v2`: `43072ms`
- Stage 3 status:
  - source report: `G:\Rust\oh-paa\.worktrees\live-replay-quality-opt\docs\superpowers\archives\2026-04-23-candidate-step2-dashscope-qwen-plus-hardened-stage3-report.json`
  - source log: `G:\Rust\oh-paa\.worktrees\live-replay-quality-opt\docs\superpowers\archives\2026-04-23-candidate-step2-dashscope-qwen-plus-hardened-stage3-run.log`
  - `schema_hit_rate = 0.8666666666666667`
  - `cross_step_consistency_rate = 0.6`
  - `decision_tree_completeness = 0.8`
  - `key_level_completeness = 1.0`
  - `signal_bar_completeness = 1.0`
  - `bull_bear_dual_path_completeness = 0.75`
  - `step_runs = 15`
  - first failing step: `shared_bar_analysis:v2` on `crypto-btcusd-2026-04-18t08-15z`
  - failure category: `schema_validation_failure`
  - schema error: `market_story` collapsed into a plain string instead of an object
  - additional failure:
    - sample `crypto-btcusd-2026-04-18t08-45z`
    - step `shared_pa_state_bar:v1`
    - failure category: `outbound_failure`
    - provider error: `error sending request for url (https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions)`
- Diagnostics hardening after Stage 3:
  - `replay_analysis` and `replay_probe` CLI binaries now initialize tracing
  - CLI startup logs are forced to stderr so stdout report capture remains clean
  - regression test: `cargo test -p pa-app replay_analysis_binary_emits_startup_log_to_stderr`
- Next adjustment:
  - tighten `shared_bar_analysis_v2` prompt contract again so every required top-level section must remain an object
  - explicitly name `market_story` as object-only to target the exact Stage 3 schema drift

## Working Conclusion

- The strongest candidate remains `qwen-plus -> deepseek-reasoner -> deepseek-reasoner -> deepseek-chat`, but it has not yet met the required two-run confirmation gate.
- `shared_bar_analysis:v2` needed contract hardening before further model comparisons were meaningful.
- The revised `step2=qwen-plus` candidate is now the strongest active alternative because it preserved `1.0` single-sample scores while cutting the target step 2 latency and avoiding the immediate schema drift seen earlier.
- The first `5-sample` run for `step2=qwen-plus` still failed, but the remaining schema drift is now narrowed to a single top-level section (`market_story`) plus one transient step 1 transport failure.
- Warmup execution cost is materially larger than target-step cost: each single-sample Stage 2 run performs `8` warmup bars x `2` LLM steps before the four target steps, so `15-20` minute wall-clock runtime is plausible even without an actual stall.
