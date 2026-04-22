use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use pa_core::Timeframe;
use pa_market::{BackfillCanonicalKlinesRequest, backfill_canonical_klines};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    router::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/market/backfill", post(backfill_market_data))
}

#[derive(Debug, Deserialize)]
struct BackfillMarketRequest {
    instrument_id: Uuid,
    timeframe: String,
    limit: Option<usize>,
}

async fn backfill_market_data(
    State(state): State<AppState>,
    Json(request): Json<BackfillMarketRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let runtime = state
        .market_runtime
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("market runtime is not configured"))?;
    let timeframe = request.timeframe.parse::<Timeframe>()?;
    let context = runtime
        .instrument_repository
        .resolve_market_data_context(request.instrument_id)
        .await?;
    let primary_provider = context.policy.kline_primary.as_str();
    let fallback_provider = context
        .policy
        .kline_fallback
        .as_deref()
        .unwrap_or(primary_provider);
    let primary_binding = context.binding_for_provider(primary_provider)?;
    let fallback_binding = context.binding_for_provider(fallback_provider)?;
    let limit = request.limit.unwrap_or(200);

    backfill_canonical_klines(
        runtime.provider_router.as_ref(),
        runtime.canonical_kline_repository.as_ref(),
        BackfillCanonicalKlinesRequest {
            instrument_id: context.instrument.id,
            primary_provider_symbol: &primary_binding.provider_symbol,
            fallback_provider_symbol: &fallback_binding.provider_symbol,
            timeframe,
            limit,
            primary_provider,
            fallback_provider,
            market_code: Some(&context.market.code),
            market_timezone: Some(&context.market.timezone),
        },
    )
    .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "status": "accepted",
            "instrument_id": context.instrument.id,
            "timeframe": timeframe.as_str(),
            "primary_provider": primary_provider,
            "primary_provider_symbol": primary_binding.provider_symbol,
            "fallback_provider": fallback_provider,
            "fallback_provider_symbol": fallback_binding.provider_symbol,
            "limit": limit,
        })),
    ))
}
