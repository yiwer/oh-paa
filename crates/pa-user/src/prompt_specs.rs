use pa_orchestrator::{AnalysisBarState, PromptResultSemantics, PromptSpec, RetryPolicyClass};

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

pub fn user_position_advice_v1() -> PromptSpec {
    PromptSpec {
        prompt_key: USER_POSITION_ADVICE_PROMPT_METADATA.prompt_key.to_string(),
        prompt_version: USER_POSITION_ADVICE_PROMPT_METADATA
            .prompt_version
            .to_string(),
        task_type: USER_POSITION_ADVICE_PROMPT_METADATA.task_type.to_string(),
        system_prompt: "Map shared PA structure to the user's position. Return JSON only."
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
