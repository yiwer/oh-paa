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

pub(crate) const USER_POSITION_ADVICE_PROMPT_METADATA: PromptMetadata = PromptMetadata {
    prompt_key: "user_position_advice",
    prompt_version: "v1",
    task_type: "user_position_advice",
    input_schema_version: "v1",
    output_schema_version: "v1",
};

pub(crate) const USER_POSITION_ADVICE_V2_PROMPT_METADATA: PromptMetadata = PromptMetadata {
    prompt_key: "user_position_advice",
    prompt_version: "v2",
    task_type: "user_position_advice",
    input_schema_version: "v2",
    output_schema_version: "v2",
};

pub fn user_position_advice_v1() -> PromptSpec {
    PromptSpec {
        prompt_key: USER_POSITION_ADVICE_PROMPT_METADATA.prompt_key.to_string(),
        prompt_version: USER_POSITION_ADVICE_PROMPT_METADATA
            .prompt_version
            .to_string(),
        task_type: USER_POSITION_ADVICE_PROMPT_METADATA.task_type.to_string(),
        system_prompt: "Map shared PA structure to the user's position using shared_daily_context_json first, shared_bar_analysis_json second, and shared_pa_state_json as supporting evidence. Return JSON only."
            .to_string(),
        input_schema_version: USER_POSITION_ADVICE_PROMPT_METADATA
            .input_schema_version
            .to_string(),
        output_schema_version: USER_POSITION_ADVICE_PROMPT_METADATA
            .output_schema_version
            .to_string(),
        output_json_schema: serde_json::json!({
            "type": "object",
            "required": [
                "position_state",
                "market_read_through",
                "bullish_path_for_user",
                "bearish_path_for_user",
                "hold_reduce_exit_conditions",
                "risk_control_levels",
                "invalidations",
                "action_candidates"
            ],
            "properties": {
                "position_state": { "type": "object" },
                "market_read_through": { "type": "object" },
                "bullish_path_for_user": { "type": "object" },
                "bearish_path_for_user": { "type": "object" },
                "hold_reduce_exit_conditions": { "type": "object" },
                "risk_control_levels": { "type": "object" },
                "invalidations": { "type": "object" },
                "action_candidates": { "type": "object" }
            }
        }),
        retry_policy_class: RetryPolicyClass::LlmStructuredOutput,
        result_semantics: PromptResultSemantics::UserPrivateAsset,
        bar_state_support: vec![AnalysisBarState::Open, AnalysisBarState::Closed],
    }
}

pub fn user_position_advice_v2() -> AnalysisStepSpec {
    AnalysisStepSpec {
        step_key: USER_POSITION_ADVICE_V2_PROMPT_METADATA
            .prompt_key
            .to_string(),
        step_version: USER_POSITION_ADVICE_V2_PROMPT_METADATA
            .prompt_version
            .to_string(),
        task_type: USER_POSITION_ADVICE_V2_PROMPT_METADATA
            .task_type
            .to_string(),
        input_schema_version: USER_POSITION_ADVICE_V2_PROMPT_METADATA
            .input_schema_version
            .to_string(),
        output_schema_version: USER_POSITION_ADVICE_V2_PROMPT_METADATA
            .output_schema_version
            .to_string(),
        output_json_schema: serde_json::json!({
            "type": "object",
            "required": [
                "position_state",
                "market_read_through",
                "bullish_path_for_user",
                "bearish_path_for_user",
                "hold_reduce_exit_conditions",
                "risk_control_levels",
                "invalidations",
                "action_candidates"
            ],
            "properties": {
                "position_state": { "type": "object" },
                "market_read_through": { "type": "object" },
                "bullish_path_for_user": { "type": "object" },
                "bearish_path_for_user": { "type": "object" },
                "hold_reduce_exit_conditions": { "type": "object" },
                "risk_control_levels": { "type": "object" },
                "invalidations": { "type": "object" },
                "action_candidates": { "type": "object" }
            }
        }),
        result_semantics: PromptResultSemantics::UserPrivateAsset,
        bar_state_support: vec![AnalysisBarState::Open, AnalysisBarState::Closed],
        dependency_policy: "requires_shared_daily_shared_bar_and_pa_state".to_string(),
    }
}

pub fn user_position_advice_prompt_v2() -> PromptTemplateSpec {
    PromptTemplateSpec {
        step_key: USER_POSITION_ADVICE_V2_PROMPT_METADATA
            .prompt_key
            .to_string(),
        step_version: USER_POSITION_ADVICE_V2_PROMPT_METADATA
            .prompt_version
            .to_string(),
        system_prompt: "Map shared PA structure to the user's position using shared_daily_context_json first, shared_bar_analysis_json second, and shared_pa_state_json as supporting evidence. Return JSON only."
            .to_string(),
        developer_instructions: vec![
            "Use the shared_daily_context_json to anchor higher-timeframe bias before discussing the current bar."
                .to_string(),
            "Translate shared_bar_analysis_json and shared_pa_state_json into concrete user-specific scenarios, controls, and invalidations."
                .to_string(),
            "Return exactly one JSON object with these top-level keys present every time: position_state, market_read_through, bullish_path_for_user, bearish_path_for_user, hold_reduce_exit_conditions, risk_control_levels, invalidations, action_candidates."
                .to_string(),
            "Do not rename schema sections. Use position_state instead of user_position, market_read_through instead of current_bar_context or higher_timeframe_bias, and keep all required top-level keys as objects."
                .to_string(),
            "If evidence is mixed, keep every required top-level object and express uncertainty inside those objects instead of omitting sections."
                .to_string(),
            "Use this minimum output skeleton and expand each object with evidence-driven details: {\"position_state\":{},\"market_read_through\":{},\"bullish_path_for_user\":{},\"bearish_path_for_user\":{},\"hold_reduce_exit_conditions\":{},\"risk_control_levels\":{},\"invalidations\":{},\"action_candidates\":{}}"
                .to_string(),
            "Prefer compact outputs: concise bullets or short phrases; avoid long narrative paragraphs unless critical for risk control clarity."
                .to_string(),
            "Return JSON only and include every required top-level field.".to_string(),
        ],
    }
}
