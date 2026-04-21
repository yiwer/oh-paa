use pa_orchestrator::{AnalysisBarState, PromptResultSemantics, PromptSpec, RetryPolicyClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PromptMetadata {
    pub prompt_key: &'static str,
    pub prompt_version: &'static str,
    pub task_type: &'static str,
    pub input_schema_version: &'static str,
    pub output_schema_version: &'static str,
}

pub(crate) const SHARED_BAR_ANALYSIS_PROMPT_METADATA: PromptMetadata = PromptMetadata {
    prompt_key: "shared_bar_analysis",
    prompt_version: "v1",
    task_type: "shared_bar_analysis",
    input_schema_version: "v1",
    output_schema_version: "v1",
};

pub(crate) const SHARED_DAILY_CONTEXT_PROMPT_METADATA: PromptMetadata = PromptMetadata {
    prompt_key: "shared_daily_context",
    prompt_version: "v1",
    task_type: "shared_daily_context",
    input_schema_version: "v1",
    output_schema_version: "v1",
};

pub fn shared_bar_analysis_v1() -> PromptSpec {
    PromptSpec {
        prompt_key: SHARED_BAR_ANALYSIS_PROMPT_METADATA.prompt_key.to_string(),
        prompt_version: SHARED_BAR_ANALYSIS_PROMPT_METADATA.prompt_version.to_string(),
        task_type: SHARED_BAR_ANALYSIS_PROMPT_METADATA.task_type.to_string(),
        system_prompt: "You are a price-action analyst. Produce strict JSON and include both bullish and bearish scenarios."
            .to_string(),
        input_schema_version: SHARED_BAR_ANALYSIS_PROMPT_METADATA
            .input_schema_version
            .to_string(),
        output_schema_version: SHARED_BAR_ANALYSIS_PROMPT_METADATA
            .output_schema_version
            .to_string(),
        output_json_schema: serde_json::json!({
            "type": "object",
            "required": [
                "bar_state",
                "bar_classification",
                "bullish_case",
                "bearish_case",
                "two_sided_summary",
                "nearby_levels",
                "signal_strength",
                "continuation_scenarios",
                "reversal_scenarios",
                "invalidation_levels",
                "execution_bias_notes"
            ],
            "properties": {
                "bar_state": {
                    "type": "string",
                    "enum": ["open", "closed"]
                },
                "bar_classification": { "type": "object" },
                "bullish_case": { "type": "object" },
                "bearish_case": { "type": "object" },
                "two_sided_summary": { "type": "object" },
                "nearby_levels": { "type": "object" },
                "signal_strength": { "type": "object" },
                "continuation_scenarios": { "type": "object" },
                "reversal_scenarios": { "type": "object" },
                "invalidation_levels": { "type": "object" },
                "execution_bias_notes": { "type": "object" }
            }
        }),
        retry_policy_class: RetryPolicyClass::LlmStructuredOutput,
        result_semantics: PromptResultSemantics::SharedAsset,
        bar_state_support: vec![AnalysisBarState::Open, AnalysisBarState::Closed],
    }
}

pub fn shared_daily_context_v1() -> PromptSpec {
    PromptSpec {
        prompt_key: SHARED_DAILY_CONTEXT_PROMPT_METADATA.prompt_key.to_string(),
        prompt_version: SHARED_DAILY_CONTEXT_PROMPT_METADATA.prompt_version.to_string(),
        task_type: SHARED_DAILY_CONTEXT_PROMPT_METADATA.task_type.to_string(),
        system_prompt: "You are a price-action analyst. Produce strict JSON with explicit PA decision-tree state."
            .to_string(),
        input_schema_version: SHARED_DAILY_CONTEXT_PROMPT_METADATA
            .input_schema_version
            .to_string(),
        output_schema_version: SHARED_DAILY_CONTEXT_PROMPT_METADATA
            .output_schema_version
            .to_string(),
        output_json_schema: serde_json::json!({
            "type": "object",
            "required": [
                "market_background",
                "market_structure",
                "key_support_levels",
                "key_resistance_levels",
                "signal_bars",
                "candle_patterns",
                "decision_tree_nodes",
                "liquidity_context",
                "risk_notes",
                "scenario_map"
            ],
            "properties": {
                "market_background": { "type": "object" },
                "market_structure": { "type": "object" },
                "key_support_levels": { "type": "object" },
                "key_resistance_levels": { "type": "object" },
                "signal_bars": { "type": "object" },
                "candle_patterns": { "type": "object" },
                "decision_tree_nodes": {
                    "type": "object",
                    "required": [
                        "trend_context",
                        "location_context",
                        "signal_quality",
                        "confirmation_state",
                        "invalidation_conditions"
                    ],
                    "properties": {
                        "trend_context": { "type": "object" },
                        "location_context": { "type": "object" },
                        "signal_quality": { "type": "object" },
                        "confirmation_state": { "type": "object" },
                        "invalidation_conditions": { "type": "object" }
                    }
                },
                "liquidity_context": { "type": "object" },
                "risk_notes": { "type": "object" },
                "scenario_map": { "type": "object" }
            }
        }),
        retry_policy_class: RetryPolicyClass::LlmStructuredOutput,
        result_semantics: PromptResultSemantics::SharedAsset,
        bar_state_support: vec![AnalysisBarState::None],
    }
}
