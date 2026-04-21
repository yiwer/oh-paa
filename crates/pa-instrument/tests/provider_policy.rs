use pa_core::AppError;
use pa_instrument::models::{PolicyScope, ProviderPolicy};
use pa_instrument::service::resolve_policy;
use uuid::Uuid;

#[test]
fn resolve_policy_prefers_instrument_override_over_market_default() {
    let market_id = Uuid::new_v4();
    let instrument_id = Uuid::new_v4();
    let market_policy = ProviderPolicy::new(
        PolicyScope::Market(market_id.to_string()),
        "iex".to_string(),
        Some("polygon".to_string()),
        "sip".to_string(),
        None,
    );
    let instrument_policy = ProviderPolicy::new(
        PolicyScope::Instrument(instrument_id.to_string()),
        "alpaca".to_string(),
        Some("iex".to_string()),
        "alpaca".to_string(),
        Some("sip".to_string()),
    );

    let resolved = resolve_policy(Some(&instrument_policy), Some(&market_policy))
        .expect("instrument policy should override market policy");

    assert_eq!(resolved, instrument_policy);
    assert_eq!(resolved.scope.scope_id(), instrument_id.to_string());
}

#[test]
fn resolve_policy_returns_validation_error_when_no_policy_is_available() {
    let error = resolve_policy(None, None).expect_err("missing policies should fail");

    assert!(matches!(
        error,
        AppError::Validation { message, .. } if message.contains("provider policy")
    ));
}
