use pa_orchestrator::{
    AnalysisBarState, AnalysisStepSpec, PromptResultSemantics, PromptSpec, PromptTemplateSpec,
    RetryPolicyClass,
};

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

pub(crate) const SHARED_BAR_ANALYSIS_V2_PROMPT_METADATA: PromptMetadata = PromptMetadata {
    prompt_key: "shared_bar_analysis",
    prompt_version: "v2",
    task_type: "shared_bar_analysis",
    input_schema_version: "v2",
    output_schema_version: "v2",
};

pub(crate) const SHARED_DAILY_CONTEXT_V2_PROMPT_METADATA: PromptMetadata = PromptMetadata {
    prompt_key: "shared_daily_context",
    prompt_version: "v2",
    task_type: "shared_daily_context",
    input_schema_version: "v2",
    output_schema_version: "v2",
};

pub(crate) const SHARED_PA_STATE_BAR_PROMPT_METADATA: PromptMetadata = PromptMetadata {
    prompt_key: "shared_pa_state_bar",
    prompt_version: "v1",
    task_type: "shared_pa_state_bar",
    input_schema_version: "v1",
    output_schema_version: "v1",
};

pub fn shared_pa_state_bar_v1() -> AnalysisStepSpec {
    AnalysisStepSpec {
        step_key: SHARED_PA_STATE_BAR_PROMPT_METADATA.prompt_key.to_string(),
        step_version: SHARED_PA_STATE_BAR_PROMPT_METADATA
            .prompt_version
            .to_string(),
        task_type: SHARED_PA_STATE_BAR_PROMPT_METADATA.task_type.to_string(),
        input_schema_version: SHARED_PA_STATE_BAR_PROMPT_METADATA
            .input_schema_version
            .to_string(),
        output_schema_version: SHARED_PA_STATE_BAR_PROMPT_METADATA
            .output_schema_version
            .to_string(),
        output_json_schema: serde_json::json!({
            "type":"object",
            "required":[
                "bar_identity",
                "market_session_context",
                "bar_observation",
                "bar_shape",
                "location_context",
                "multi_timeframe_alignment",
                "support_resistance_map",
                "signal_assessment",
                "decision_tree_state",
                "evidence_log"
            ],
            "properties": {
                "bar_identity": { "type":"object" },
                "market_session_context": { "type":"object" },
                "bar_observation": { "type":"object" },
                "bar_shape": { "type":"object" },
                "location_context": { "type":"object" },
                "multi_timeframe_alignment": { "type":"object" },
                "support_resistance_map": { "type":"object" },
                "signal_assessment": { "type":"object" },
                "decision_tree_state": {
                    "type":"object",
                    "required":[
                        "trend_context",
                        "location_context",
                        "signal_quality",
                        "confirmation_state",
                        "invalidation_conditions",
                        "bias_balance"
                    ]
                },
                "evidence_log": { "type":"object" }
            }
        }),
        result_semantics: PromptResultSemantics::SharedAsset,
        bar_state_support: vec![AnalysisBarState::Closed, AnalysisBarState::Open],
        dependency_policy: "market_runtime_only".into(),
    }
}

pub fn shared_pa_state_bar_prompt_v1() -> PromptTemplateSpec {
    PromptTemplateSpec {
        step_key: SHARED_PA_STATE_BAR_PROMPT_METADATA.prompt_key.to_string(),
        step_version: SHARED_PA_STATE_BAR_PROMPT_METADATA
            .prompt_version
            .to_string(),
        system_prompt: "You are a price-action analyst. Produce strict JSON that captures reusable PA state for the target bar without final trade commentary."
            .to_string(),
        developer_instructions: vec![
            "Summarize reusable price-action state rather than directional advice.".to_string(),
            "Ground every conclusion in evidence from the provided bar and market context."
                .to_string(),
            "Return JSON only and preserve the required decision_tree_state fields."
                .to_string(),
        ],
    }
}

pub fn shared_bar_analysis_v2() -> AnalysisStepSpec {
    AnalysisStepSpec {
        step_key: SHARED_BAR_ANALYSIS_V2_PROMPT_METADATA
            .prompt_key
            .to_string(),
        step_version: SHARED_BAR_ANALYSIS_V2_PROMPT_METADATA
            .prompt_version
            .to_string(),
        task_type: SHARED_BAR_ANALYSIS_V2_PROMPT_METADATA.task_type.to_string(),
        input_schema_version: SHARED_BAR_ANALYSIS_V2_PROMPT_METADATA
            .input_schema_version
            .to_string(),
        output_schema_version: SHARED_BAR_ANALYSIS_V2_PROMPT_METADATA
            .output_schema_version
            .to_string(),
        output_json_schema: serde_json::json!({
            "type": "object",
            "required": [
                "bar_identity",
                "bar_summary",
                "market_story",
                "bullish_case",
                "bearish_case",
                "two_sided_balance",
                "key_levels",
                "signal_bar_verdict",
                "continuation_path",
                "reversal_path",
                "invalidation_map",
                "follow_through_checkpoints"
            ],
            "properties": {
                "bar_identity": { "type": "object" },
                "bar_summary": { "type": "object" },
                "market_story": { "type": "object" },
                "bullish_case": { "type": "object" },
                "bearish_case": { "type": "object" },
                "two_sided_balance": { "type": "object" },
                "key_levels": { "type": "object" },
                "signal_bar_verdict": { "type": "object" },
                "continuation_path": { "type": "object" },
                "reversal_path": { "type": "object" },
                "invalidation_map": { "type": "object" },
                "follow_through_checkpoints": { "type": "object" }
            }
        }),
        result_semantics: PromptResultSemantics::SharedAsset,
        bar_state_support: vec![AnalysisBarState::Open, AnalysisBarState::Closed],
        dependency_policy: "requires_shared_pa_state".into(),
    }
}

pub fn shared_bar_analysis_prompt_v2() -> PromptTemplateSpec {
    PromptTemplateSpec {
        step_key: SHARED_BAR_ANALYSIS_V2_PROMPT_METADATA
            .prompt_key
            .to_string(),
        step_version: SHARED_BAR_ANALYSIS_V2_PROMPT_METADATA
            .prompt_version
            .to_string(),
        system_prompt: "You are a price-action analyst. Produce strict JSON grounded in the shared PA state and preserve a balanced two-sided view."
            .to_string(),
        developer_instructions: vec![
            "Use shared_pa_state_json as the primary source of state and cite concrete evidence in each section.".to_string(),
            "Keep both bullish and bearish paths explicit, actionable, and internally consistent.".to_string(),
            "Return JSON only and include every required top-level field.".to_string(),
        ],
    }
}

pub fn shared_daily_context_v2() -> AnalysisStepSpec {
    AnalysisStepSpec {
        step_key: SHARED_DAILY_CONTEXT_V2_PROMPT_METADATA
            .prompt_key
            .to_string(),
        step_version: SHARED_DAILY_CONTEXT_V2_PROMPT_METADATA
            .prompt_version
            .to_string(),
        task_type: SHARED_DAILY_CONTEXT_V2_PROMPT_METADATA
            .task_type
            .to_string(),
        input_schema_version: SHARED_DAILY_CONTEXT_V2_PROMPT_METADATA
            .input_schema_version
            .to_string(),
        output_schema_version: SHARED_DAILY_CONTEXT_V2_PROMPT_METADATA
            .output_schema_version
            .to_string(),
        output_json_schema: serde_json::json!({
            "type": "object",
            "required": [
                "context_identity",
                "market_background",
                "dominant_structure",
                "intraday_vs_higher_timeframe_state",
                "key_support_levels",
                "key_resistance_levels",
                "signal_bars",
                "candle_pattern_map",
                "decision_tree_nodes",
                "liquidity_context",
                "scenario_map",
                "risk_notes",
                "session_playbook"
            ],
            "properties": {
                "context_identity": { "type": "object" },
                "market_background": { "type": "object" },
                "dominant_structure": { "type": "object" },
                "intraday_vs_higher_timeframe_state": { "type": "object" },
                "key_support_levels": { "type": "object" },
                "key_resistance_levels": { "type": "object" },
                "signal_bars": { "type": "object" },
                "candle_pattern_map": { "type": "object" },
                "decision_tree_nodes": {
                    "type": "object",
                    "required": [
                        "trend_context",
                        "location_context",
                        "signal_quality",
                        "confirmation_state",
                        "invalidation_conditions",
                        "path_of_least_resistance"
                    ],
                    "properties": {
                        "trend_context": { "type": "object" },
                        "location_context": { "type": "object" },
                        "signal_quality": { "type": "object" },
                        "confirmation_state": { "type": "object" },
                        "invalidation_conditions": { "type": "object" },
                        "path_of_least_resistance": { "type": "object" }
                    }
                },
                "liquidity_context": { "type": "object" },
                "scenario_map": { "type": "object" },
                "risk_notes": { "type": "object" },
                "session_playbook": { "type": "object" }
            }
        }),
        result_semantics: PromptResultSemantics::SharedAsset,
        bar_state_support: vec![AnalysisBarState::None],
        dependency_policy: "requires_shared_pa_state_optional_shared_bar".into(),
    }
}

pub fn shared_daily_context_prompt_v2() -> PromptTemplateSpec {
    PromptTemplateSpec {
        step_key: SHARED_DAILY_CONTEXT_V2_PROMPT_METADATA
            .prompt_key
            .to_string(),
        step_version: SHARED_DAILY_CONTEXT_V2_PROMPT_METADATA
            .prompt_version
            .to_string(),
        system_prompt: "You are a price-action analyst. Produce strict JSON daily context by synthesizing shared PA states, shared bar analyses, and multi-timeframe structure."
            .to_string(),
        developer_instructions: vec![
            "Prioritize alignment between intraday and higher-timeframe structure and make conflicts explicit.".to_string(),
            "Keep decision_tree_nodes complete with all required state fields and concrete invalidation logic.".to_string(),
            "Return JSON only and include every required top-level field.".to_string(),
        ],
    }
}

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
