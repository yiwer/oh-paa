# Market Gateway Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce `pa_market::MarketGateway` so workflow functions accept `&InstrumentMarketDataContext` instead of 6–9 individual strings. Make `ProviderPolicy` and `InstrumentSymbolBinding` load-bearing instead of decorative.

**Architecture:** Thin gateway in `pa-market` that resolves provider names from `ctx.policy` and provider symbols from `ctx.bindings`, then delegates to the existing `ProviderRouter`. Workflow functions (`backfill_canonical_klines`, `derive_open_bar`, `aggregate_*`) updated to accept `&InstrumentMarketDataContext`. No business-logic change. Single PR.

**Tech Stack:** Rust 2024, async-trait, tokio, sqlx (touched but not exercised in this work), `pa-instrument` (DB models + resolver), `pa-market` (provider router + workflow), `pa-api` (Axum handlers).

**Spec:** `docs/superpowers/specs/2026-04-26-market-gateway-design.md`

---

## File Structure

| File | Role |
|---|---|
| `crates/pa-instrument/Cargo.toml` | Add `test-fixtures` feature |
| `crates/pa-instrument/src/repository.rs` | Add `InstrumentMarketDataContext::fixture` builder behind feature flag |
| `crates/pa-instrument/src/lib.rs` | (No changes) |
| `crates/pa-market/Cargo.toml` | Add `pa-instrument` regular dep + dev-dep with `test-fixtures` |
| `crates/pa-market/src/lib.rs` | Add `pub mod gateway;`; re-export `MarketGateway`; remove `BackfillCanonicalKlinesRequest` and `DeriveOpenBarRequest` from re-exports |
| `crates/pa-market/src/gateway.rs` | **NEW.** `MarketGateway` struct + 3 fetch methods |
| `crates/pa-market/src/provider.rs` | Promote `fetch_klines_from` and `fetch_latest_tick_from` from private to `pub(crate)` |
| `crates/pa-market/src/session.rs` | Add `MarketSessionProfile::from_market_record` |
| `crates/pa-market/src/service.rs` | Reshape 4 workflow functions; remove 2 request structs; shrink 1 |
| `crates/pa-market/tests/gateway.rs` | **NEW.** 5+ unit tests for `MarketGateway` |
| `crates/pa-market/tests/backfill_idempotent.rs` | Update 3 call sites to new signature |
| `crates/pa-market/tests/open_bar_runtime.rs` | Update 2 call sites |
| `crates/pa-market/tests/session_aggregation.rs` | Update 3 call sites |
| `crates/pa-api/src/router.rs` | Replace `provider_router: Arc<ProviderRouter>` with `market_gateway: Arc<MarketGateway>` in `MarketRuntime` |
| `crates/pa-api/src/admin.rs` | 1 call site |
| `crates/pa-api/src/market.rs` | 4 call sites |
| `crates/pa-api/src/analysis_runtime.rs` | 6 call sites |
| `crates/pa-app/src/main.rs` | Wrap `ProviderRouter` in `MarketGateway` at startup |
| `crates/pa-app/src/replay_live.rs` | Continues using `&ProviderRouter` via `gateway.router()` (no signature change) |
| `docs/superpowers/specs/2026-04-26-observability-foundation-design.md` | Add `market` label to 5 metrics in catalog |

---

## Task 1: Add `test-fixtures` feature and fixture builder to `pa-instrument`

**Files:**
- Modify: `crates/pa-instrument/Cargo.toml`
- Modify: `crates/pa-instrument/src/repository.rs`
- Modify: `crates/pa-market/Cargo.toml`

- [ ] **Step 1: Add feature flag to `pa-instrument`**

Modify `crates/pa-instrument/Cargo.toml` to add a `[features]` section after `[dev-dependencies]`:

```toml
[features]
test-fixtures = []
```

- [ ] **Step 2: Add fixture builder behind feature flag**

Append to `crates/pa-instrument/src/repository.rs` (just before the existing `#[cfg(test)] mod tests` block):

```rust
#[cfg(any(test, feature = "test-fixtures"))]
impl InstrumentMarketDataContext {
    /// Build an in-memory `InstrumentMarketDataContext` for tests.
    ///
    /// `policy_primary` and `policy_fallback` are provider names. For each
    /// non-`None` entry, an `InstrumentSymbolBinding` is generated using
    /// `format!("{}-{}", instrument_symbol, provider)` as the
    /// `provider_symbol`.
    pub fn fixture(
        market_code: &str,
        market_timezone: &str,
        instrument_symbol: &str,
        kline_primary: &str,
        kline_fallback: Option<&str>,
        tick_primary: &str,
        tick_fallback: Option<&str>,
    ) -> Self {
        use chrono::Utc;

        let market_id = uuid::Uuid::new_v4();
        let instrument_id = uuid::Uuid::new_v4();
        let now = Utc::now();

        let market = crate::models::Market {
            id: market_id,
            code: market_code.to_string(),
            name: market_code.to_string(),
            timezone: market_timezone.to_string(),
            created_at: now,
            updated_at: now,
        };
        let instrument = crate::models::Instrument {
            id: instrument_id,
            market_id,
            symbol: instrument_symbol.to_string(),
            name: instrument_symbol.to_string(),
            instrument_type: "equity".to_string(),
            created_at: now,
            updated_at: now,
        };

        let mut bindings = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for provider in [
            Some(kline_primary),
            kline_fallback,
            Some(tick_primary),
            tick_fallback,
        ]
        .into_iter()
        .flatten()
        {
            if !seen.insert(provider.to_string()) {
                continue;
            }
            bindings.push(crate::models::InstrumentSymbolBinding {
                id: uuid::Uuid::new_v4(),
                instrument_id,
                provider: provider.to_string(),
                provider_symbol: format!("{instrument_symbol}-{provider}"),
                created_at: now,
            });
        }

        let policy = crate::models::ProviderPolicy::new(
            crate::models::PolicyScope::Market(market_id.to_string()),
            kline_primary.to_string(),
            kline_fallback.map(|s| s.to_string()),
            tick_primary.to_string(),
            tick_fallback.map(|s| s.to_string()),
        );

        Self {
            market,
            instrument,
            policy,
            bindings,
        }
    }
}
```

- [ ] **Step 3: Add `pa-instrument` dependency to `pa-market`**

Modify `crates/pa-market/Cargo.toml`. In `[dependencies]` add a line for `pa-instrument` (alphabetic with other path deps; `pa-core` is the existing reference point):

```toml
pa-instrument = { path = "../pa-instrument" }
```

In `[dev-dependencies]` add the same crate with the test-fixtures feature enabled:

```toml
pa-instrument = { path = "../pa-instrument", features = ["test-fixtures"] }
```

- [ ] **Step 4: Verify the workspace compiles**

Run: `cargo check -p pa-instrument -p pa-market --tests`
Expected: clean build, no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/pa-instrument/Cargo.toml crates/pa-instrument/src/repository.rs crates/pa-market/Cargo.toml
git commit -m "feat(pa-instrument): add test-fixtures feature with InstrumentMarketDataContext::fixture"
```

---

## Task 2: Promote `ProviderRouter` single-provider methods to `pub(crate)`

**Files:**
- Modify: `crates/pa-market/src/provider.rs`

- [ ] **Step 1: Change visibility of `fetch_klines_from` and `fetch_latest_tick_from`**

In `crates/pa-market/src/provider.rs`, change both method declarations from `async fn` (private) to `pub(crate) async fn`. Locate `async fn fetch_klines_from` and `async fn fetch_latest_tick_from`:

```rust
pub(crate) async fn fetch_klines_from(
    &self,
    provider_name: &str,
    provider_symbol: &str,
    timeframe: Timeframe,
    limit: usize,
) -> Result<Vec<ProviderKline>, AppError> {
    // ... unchanged body
}

pub(crate) async fn fetch_latest_tick_from(
    &self,
    provider_name: &str,
    provider_symbol: &str,
) -> Result<ProviderTick, AppError> {
    // ... unchanged body
}
```

- [ ] **Step 2: Verify all existing pa-market tests still pass**

Run: `cargo test -p pa-market`
Expected: all green, no regressions.

- [ ] **Step 3: Commit**

```bash
git add crates/pa-market/src/provider.rs
git commit -m "refactor(pa-market): expose ProviderRouter single-provider helpers to crate"
```

---

## Task 3: Scaffold `MarketGateway` and implement `fetch_klines` (TDD)

**Files:**
- Create: `crates/pa-market/src/gateway.rs`
- Create: `crates/pa-market/tests/gateway.rs`
- Modify: `crates/pa-market/src/lib.rs`

- [ ] **Step 1: Write the failing test file**

Create `crates/pa-market/tests/gateway.rs`:

```rust
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use pa_core::{AppError, Timeframe};
use pa_instrument::InstrumentMarketDataContext;
use pa_market::{MarketDataProvider, MarketGateway, ProviderKline, ProviderRouter, ProviderTick};

struct StubProvider {
    name: &'static str,
    klines: Result<Vec<ProviderKline>, AppError>,
    tick: Result<ProviderTick, AppError>,
    kline_calls: Arc<AtomicUsize>,
    tick_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl MarketDataProvider for StubProvider {
    fn name(&self) -> &'static str {
        self.name
    }
    async fn fetch_klines(
        &self,
        _provider_symbol: &str,
        _timeframe: Timeframe,
        _limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        self.kline_calls.fetch_add(1, Ordering::SeqCst);
        match &self.klines {
            Ok(k) => Ok(k.clone()),
            Err(_) => Err(AppError::Provider {
                message: format!("{} kline failed", self.name),
                source: None,
            }),
        }
    }
    async fn fetch_latest_tick(&self, _provider_symbol: &str) -> Result<ProviderTick, AppError> {
        self.tick_calls.fetch_add(1, Ordering::SeqCst);
        match &self.tick {
            Ok(t) => Ok(t.clone()),
            Err(_) => Err(AppError::Provider {
                message: format!("{} tick failed", self.name),
                source: None,
            }),
        }
    }
    async fn healthcheck(&self) -> Result<(), AppError> {
        Ok(())
    }
}

fn ok_klines_provider(name: &'static str, klines: Vec<ProviderKline>) -> Arc<StubProvider> {
    Arc::new(StubProvider {
        name,
        klines: Ok(klines),
        tick: Err(AppError::Provider {
            message: "tick not exercised".into(),
            source: None,
        }),
        kline_calls: Arc::new(AtomicUsize::new(0)),
        tick_calls: Arc::new(AtomicUsize::new(0)),
    })
}

fn err_klines_provider(name: &'static str) -> Arc<StubProvider> {
    Arc::new(StubProvider {
        name,
        klines: Err(AppError::Provider {
            message: "boom".into(),
            source: None,
        }),
        tick: Err(AppError::Provider {
            message: "boom".into(),
            source: None,
        }),
        kline_calls: Arc::new(AtomicUsize::new(0)),
        tick_calls: Arc::new(AtomicUsize::new(0)),
    })
}

#[tokio::test]
async fn fetch_klines_returns_primary_provider_when_primary_succeeds() {
    let primary = ok_klines_provider("primary", vec![ProviderKline::fixture()]);
    let fallback = ok_klines_provider("fallback", vec![ProviderKline::fixture()]);

    let mut router = ProviderRouter::default();
    router.insert(primary.clone());
    router.insert(fallback.clone());
    let gateway = MarketGateway::new(router);

    let ctx = InstrumentMarketDataContext::fixture(
        "cn-a",
        "Asia/Shanghai",
        "000001",
        "primary",
        Some("fallback"),
        "primary",
        Some("fallback"),
    );

    let routed = gateway
        .fetch_klines(&ctx, Timeframe::M15, 100)
        .await
        .expect("primary should satisfy");

    assert_eq!(routed.provider_name, "primary");
    assert_eq!(routed.klines.len(), 1);
}
```

- [ ] **Step 2: Run the test to verify it fails to compile**

Run: `cargo test -p pa-market --test gateway`
Expected: compile error — `MarketGateway` not found.

- [ ] **Step 3: Add `pub mod gateway;` and re-export to lib.rs**

In `crates/pa-market/src/lib.rs`, add `pub mod gateway;` after `pub mod provider;` (line ordering preserved). Add `pub use gateway::MarketGateway;` after the existing `pub use provider::...` line:

```rust
pub mod gateway;
// ...
pub use gateway::MarketGateway;
```

- [ ] **Step 4: Implement `MarketGateway` minimally**

Create `crates/pa-market/src/gateway.rs`:

```rust
use chrono::{DateTime, Utc};
use pa_core::{AppError, Timeframe};
use pa_instrument::InstrumentMarketDataContext;

use crate::provider::{
    HistoricalKlineQuery, ProviderRouter, RoutedKlines, RoutedTick,
};

pub struct MarketGateway {
    router: ProviderRouter,
}

impl MarketGateway {
    pub fn new(router: ProviderRouter) -> Self {
        Self { router }
    }

    pub fn router(&self) -> &ProviderRouter {
        &self.router
    }

    pub async fn fetch_klines(
        &self,
        ctx: &InstrumentMarketDataContext,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<RoutedKlines, AppError> {
        let primary = ctx.policy.kline_primary.as_str();
        let primary_symbol = ctx.binding_for_provider(primary)?.provider_symbol.clone();

        match ctx.policy.kline_fallback.as_deref() {
            Some(fallback) => {
                let fallback_symbol =
                    ctx.binding_for_provider(fallback)?.provider_symbol.clone();
                self.router
                    .fetch_klines_with_fallback_source(
                        primary,
                        fallback,
                        &primary_symbol,
                        &fallback_symbol,
                        timeframe,
                        limit,
                    )
                    .await
            }
            None => {
                let klines = self
                    .router
                    .fetch_klines_from(primary, &primary_symbol, timeframe, limit)
                    .await?;
                Ok(RoutedKlines {
                    provider_name: primary.to_string(),
                    klines,
                })
            }
        }
    }

    pub async fn fetch_klines_window(
        &self,
        ctx: &InstrumentMarketDataContext,
        timeframe: Timeframe,
        start_open_time: Option<DateTime<Utc>>,
        end_close_time: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<RoutedKlines, AppError> {
        let primary = ctx.policy.kline_primary.as_str();
        let primary_symbol = ctx.binding_for_provider(primary)?.provider_symbol.clone();

        let klines = self
            .router
            .fetch_klines_window_from(
                primary,
                HistoricalKlineQuery {
                    provider_symbol: primary_symbol,
                    timeframe,
                    start_open_time,
                    end_close_time,
                    limit,
                },
            )
            .await?;
        Ok(RoutedKlines {
            provider_name: primary.to_string(),
            klines,
        })
    }

    pub async fn fetch_latest_tick(
        &self,
        ctx: &InstrumentMarketDataContext,
    ) -> Result<RoutedTick, AppError> {
        let primary = ctx.policy.tick_primary.as_str();
        let primary_symbol = ctx.binding_for_provider(primary)?.provider_symbol.clone();

        match ctx.policy.tick_fallback.as_deref() {
            Some(fallback) => {
                let fallback_symbol =
                    ctx.binding_for_provider(fallback)?.provider_symbol.clone();
                self.router
                    .fetch_latest_tick_with_fallback_source(
                        primary,
                        fallback,
                        &primary_symbol,
                        &fallback_symbol,
                    )
                    .await
            }
            None => {
                let tick = self
                    .router
                    .fetch_latest_tick_from(primary, &primary_symbol)
                    .await?;
                Ok(RoutedTick {
                    provider_name: primary.to_string(),
                    tick,
                })
            }
        }
    }
}
```

- [ ] **Step 5: Run the test — should pass**

Run: `cargo test -p pa-market --test gateway`
Expected: 1 test passes.

- [ ] **Step 6: Commit**

```bash
git add crates/pa-market/src/gateway.rs crates/pa-market/src/lib.rs crates/pa-market/tests/gateway.rs
git commit -m "feat(pa-market): add MarketGateway with fetch_klines/window/latest_tick"
```

---

## Task 4: Add fallback + error coverage tests for `MarketGateway`

**Files:**
- Modify: `crates/pa-market/tests/gateway.rs`

- [ ] **Step 1: Append fallback test**

Append to `crates/pa-market/tests/gateway.rs`:

```rust
#[tokio::test]
async fn fetch_klines_falls_back_when_primary_returns_empty() {
    let primary = ok_klines_provider("primary", Vec::new());
    let fallback = ok_klines_provider("fallback", vec![ProviderKline::fixture()]);

    let mut router = ProviderRouter::default();
    router.insert(primary);
    router.insert(fallback);
    let gateway = MarketGateway::new(router);

    let ctx = InstrumentMarketDataContext::fixture(
        "cn-a",
        "Asia/Shanghai",
        "000001",
        "primary",
        Some("fallback"),
        "primary",
        Some("fallback"),
    );

    let routed = gateway
        .fetch_klines(&ctx, Timeframe::M15, 100)
        .await
        .expect("fallback should satisfy");

    assert_eq!(routed.provider_name, "fallback");
    assert_eq!(routed.klines.len(), 1);
}
```

- [ ] **Step 2: Append missing-binding test**

```rust
#[tokio::test]
async fn fetch_klines_returns_validation_when_binding_missing() {
    let primary = ok_klines_provider("primary", vec![ProviderKline::fixture()]);
    let mut router = ProviderRouter::default();
    router.insert(primary);
    let gateway = MarketGateway::new(router);

    // Policy points at "ghost" but no binding for that provider exists.
    let mut ctx = InstrumentMarketDataContext::fixture(
        "cn-a",
        "Asia/Shanghai",
        "000001",
        "primary",
        None,
        "primary",
        None,
    );
    ctx.policy.kline_primary = "ghost".to_string();

    let err = gateway
        .fetch_klines(&ctx, Timeframe::M15, 100)
        .await
        .expect_err("missing binding should error");

    match err {
        AppError::Validation { message, .. } => {
            assert!(message.contains("missing provider binding"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}
```

- [ ] **Step 3: Append missing-provider-in-router test**

```rust
#[tokio::test]
async fn fetch_klines_returns_validation_when_provider_not_registered() {
    let router = ProviderRouter::default(); // empty router
    let gateway = MarketGateway::new(router);

    let ctx = InstrumentMarketDataContext::fixture(
        "cn-a",
        "Asia/Shanghai",
        "000001",
        "primary",
        None,
        "primary",
        None,
    );

    let err = gateway
        .fetch_klines(&ctx, Timeframe::M15, 100)
        .await
        .expect_err("unregistered provider should error");

    match err {
        AppError::Validation { message, .. } => {
            assert!(message.contains("provider `primary` is not registered"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}
```

- [ ] **Step 4: Append no-fallback-and-primary-fails test**

```rust
#[tokio::test]
async fn fetch_klines_no_fallback_surfaces_primary_failure() {
    let primary = err_klines_provider("primary");
    let mut router = ProviderRouter::default();
    router.insert(primary);
    let gateway = MarketGateway::new(router);

    let ctx = InstrumentMarketDataContext::fixture(
        "cn-a",
        "Asia/Shanghai",
        "000001",
        "primary",
        None,
        "primary",
        None,
    );

    let err = gateway
        .fetch_klines(&ctx, Timeframe::M15, 100)
        .await
        .expect_err("primary failure with no fallback should surface");

    match err {
        AppError::Provider { message, .. } => {
            assert!(message.contains("primary kline failed"));
        }
        other => panic!("expected provider error, got {other:?}"),
    }
}
```

- [ ] **Step 5: Append latest-tick happy-path test**

```rust
fn sample_tick(price: &str) -> ProviderTick {
    ProviderTick {
        price: price.parse().expect("decimal parses"),
        size: None,
        tick_time: chrono::DateTime::parse_from_rfc3339("2024-01-02T09:30:00Z")
            .expect("rfc3339 parses")
            .with_timezone(&chrono::Utc),
    }
}

#[tokio::test]
async fn fetch_latest_tick_returns_primary_when_primary_succeeds() {
    let primary = Arc::new(StubProvider {
        name: "primary",
        klines: Err(AppError::Provider {
            message: "klines not exercised".into(),
            source: None,
        }),
        tick: Ok(sample_tick("10.5")),
        kline_calls: Arc::new(AtomicUsize::new(0)),
        tick_calls: Arc::new(AtomicUsize::new(0)),
    });

    let mut router = ProviderRouter::default();
    router.insert(primary);
    let gateway = MarketGateway::new(router);

    let ctx = InstrumentMarketDataContext::fixture(
        "cn-a",
        "Asia/Shanghai",
        "000001",
        "primary",
        None,
        "primary",
        None,
    );

    let routed = gateway
        .fetch_latest_tick(&ctx)
        .await
        .expect("primary tick should satisfy");

    assert_eq!(routed.provider_name, "primary");
    assert_eq!(routed.tick.price, "10.5".parse().unwrap());
}
```

- [ ] **Step 6: Run all gateway tests**

Run: `cargo test -p pa-market --test gateway`
Expected: 5 tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/pa-market/tests/gateway.rs
git commit -m "test(pa-market): cover MarketGateway fallback/error/no-fallback paths"
```

---

## Task 5: Add `MarketSessionProfile::from_market_record`

**Files:**
- Modify: `crates/pa-market/src/session.rs`

- [ ] **Step 1: Append the constructor**

Add inside the existing `impl MarketSessionProfile` block in `crates/pa-market/src/session.rs`:

```rust
pub fn from_market_record(market: &pa_instrument::Market) -> Self {
    Self::from_market(Some(&market.code), Some(&market.timezone))
}
```

- [ ] **Step 2: Add a unit test**

Append to `crates/pa-market/src/session.rs` (inside existing `#[cfg(test)] mod tests` block, or create one if absent):

```rust
#[cfg(test)]
mod from_market_record_tests {
    use super::{MarketSessionKind, MarketSessionProfile};
    use chrono::Utc;
    use pa_instrument::Market;
    use uuid::Uuid;

    #[test]
    fn from_market_record_matches_from_market_strings() {
        let now = Utc::now();
        let market = Market {
            id: Uuid::new_v4(),
            code: "cn-a".to_string(),
            name: "CN A".to_string(),
            timezone: "Asia/Shanghai".to_string(),
            created_at: now,
            updated_at: now,
        };

        let from_record = MarketSessionProfile::from_market_record(&market);
        let from_strings = MarketSessionProfile::from_market(Some("cn-a"), Some("Asia/Shanghai"));

        assert_eq!(from_record, from_strings);
        assert_eq!(from_record.kind, MarketSessionKind::CnA);
    }
}
```

- [ ] **Step 3: Run the test**

Run: `cargo test -p pa-market --lib from_market_record`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/pa-market/src/session.rs
git commit -m "feat(pa-market): add MarketSessionProfile::from_market_record"
```

---

## Task 6: Reshape `backfill_canonical_klines` and update its tests

**Files:**
- Modify: `crates/pa-market/src/service.rs`
- Modify: `crates/pa-market/src/lib.rs`
- Modify: `crates/pa-market/tests/backfill_idempotent.rs`

- [ ] **Step 1: Replace `backfill_canonical_klines` body and remove `BackfillCanonicalKlinesRequest`**

In `crates/pa-market/src/service.rs`:

1. Delete the `BackfillCanonicalKlinesRequest` struct entirely.
2. Replace the entire `pub async fn backfill_canonical_klines` function with:

```rust
pub async fn backfill_canonical_klines(
    gateway: &crate::MarketGateway,
    repository: &dyn CanonicalKlineRepository,
    ctx: &pa_instrument::InstrumentMarketDataContext,
    timeframe: Timeframe,
    limit: usize,
) -> Result<(), AppError> {
    let session_profile = MarketSessionProfile::from_market_record(&ctx.market);
    let routed = gateway.fetch_klines(ctx, timeframe, limit).await?;

    for bar in routed.klines {
        let normalized = normalize_kline(bar)?;
        if normalized.close_time > Utc::now() {
            continue;
        }
        if !session_profile.accepts_bar_open(timeframe, normalized.open_time) {
            continue;
        }

        repository
            .upsert_canonical_kline(CanonicalKlineRow {
                instrument_id: ctx.instrument.id,
                timeframe,
                open_time: normalized.open_time,
                close_time: normalized.close_time,
                open: normalized.open,
                high: normalized.high,
                low: normalized.low,
                close: normalized.close,
                volume: normalized.volume,
                source_provider: routed.provider_name.to_string(),
            })
            .await?;
    }

    Ok(())
}
```

3. Remove the now-unused `use crate::provider::ProviderRouter;` import if it becomes unused after Task 7+8 (leave it alone for now if `derive_open_bar` still uses it).

- [ ] **Step 2: Update lib.rs re-export**

In `crates/pa-market/src/lib.rs`, change the `pub use service::` line to remove `BackfillCanonicalKlinesRequest`:

```rust
pub use service::{
    AggregateCanonicalKlinesRequest, AggregatedKline,
    DeriveOpenBarRequest, DerivedOpenBar, aggregate_canonical_klines, aggregate_replay_window_rows,
    backfill_canonical_klines, derive_open_bar, list_canonical_klines,
};
```

(Keep `DeriveOpenBarRequest` for now — Task 7 removes it.)

- [ ] **Step 3: Rewrite `tests/backfill_idempotent.rs`**

Replace the contents of `crates/pa-market/tests/backfill_idempotent.rs`:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use pa_core::{AppError, Timeframe};
use pa_instrument::InstrumentMarketDataContext;
use pa_market::{
    InMemoryCanonicalKlineRepository, MarketDataProvider, MarketGateway, ProviderKline,
    ProviderRouter, ProviderTick, backfill_canonical_klines,
};

struct StubProvider {
    name: &'static str,
    klines: Result<Vec<ProviderKline>, AppError>,
}

#[async_trait]
impl MarketDataProvider for StubProvider {
    fn name(&self) -> &'static str {
        self.name
    }
    async fn fetch_klines(
        &self,
        _provider_symbol: &str,
        _timeframe: Timeframe,
        _limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        match &self.klines {
            Ok(k) => Ok(k.clone()),
            Err(_) => Err(AppError::Provider {
                message: format!("{} failed", self.name),
                source: None,
            }),
        }
    }
    async fn fetch_latest_tick(&self, _provider_symbol: &str) -> Result<ProviderTick, AppError> {
        unimplemented!("tick fetching is outside this backfill test")
    }
    async fn healthcheck(&self) -> Result<(), AppError> {
        Ok(())
    }
}

fn ctx_for_test() -> InstrumentMarketDataContext {
    InstrumentMarketDataContext::fixture(
        "continuous-utc",
        "UTC",
        "000001",
        "primary",
        Some("fallback"),
        "primary",
        Some("fallback"),
    )
}

fn gateway_with(primary: Vec<ProviderKline>, fallback: Vec<ProviderKline>) -> MarketGateway {
    let mut router = ProviderRouter::default();
    router.insert(Arc::new(StubProvider {
        name: "primary",
        klines: Ok(primary),
    }));
    router.insert(Arc::new(StubProvider {
        name: "fallback",
        klines: Ok(fallback),
    }));
    MarketGateway::new(router)
}

#[tokio::test]
async fn repeated_backfill_upserts_canonical_rows_by_instrument_timeframe_and_open_time() {
    let ctx = ctx_for_test();
    let klines = vec![ProviderKline::fixture(), ProviderKline::fixture()];
    let repository = InMemoryCanonicalKlineRepository::default();
    let gateway = gateway_with(klines, Vec::new());

    backfill_canonical_klines(&gateway, &repository, &ctx, Timeframe::M15, 100)
        .await
        .expect("first backfill should succeed");
    backfill_canonical_klines(&gateway, &repository, &ctx, Timeframe::M15, 100)
        .await
        .expect("repeat backfill should still succeed");

    let rows = repository.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].instrument_id, ctx.instrument.id);
    assert_eq!(rows[0].timeframe, Timeframe::M15);
    assert_eq!(rows[0].open_time, ProviderKline::fixture().open_time);
}

#[tokio::test]
async fn backfill_skips_bars_whose_close_time_is_still_in_the_future() {
    let ctx = ctx_for_test();
    let mut future_bar = ProviderKline::fixture();
    future_bar.close_time = Utc::now() + Duration::minutes(15);

    let repository = InMemoryCanonicalKlineRepository::default();
    let gateway = gateway_with(vec![future_bar], Vec::new());

    backfill_canonical_klines(&gateway, &repository, &ctx, Timeframe::M15, 100)
        .await
        .expect("future bars should be skipped without failing backfill");

    assert!(repository.rows().is_empty());
}

#[tokio::test]
async fn backfill_persists_fallback_provider_name_when_primary_returns_empty() {
    let ctx = ctx_for_test();
    let repository = InMemoryCanonicalKlineRepository::default();
    let gateway = gateway_with(Vec::new(), vec![ProviderKline::fixture()]);

    backfill_canonical_klines(&gateway, &repository, &ctx, Timeframe::M15, 100)
        .await
        .expect("fallback backfill should succeed");

    let rows = repository.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].source_provider, "fallback");
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p pa-market --test backfill_idempotent`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pa-market/src/service.rs crates/pa-market/src/lib.rs crates/pa-market/tests/backfill_idempotent.rs
git commit -m "refactor(pa-market): backfill_canonical_klines takes MarketGateway + context"
```

---

## Task 7: Reshape `derive_open_bar` and update its tests

**Files:**
- Modify: `crates/pa-market/src/service.rs`
- Modify: `crates/pa-market/src/lib.rs`
- Modify: `crates/pa-market/tests/open_bar_runtime.rs`

- [ ] **Step 1: Replace `derive_open_bar` and remove `DeriveOpenBarRequest`**

In `crates/pa-market/src/service.rs`:

1. Delete the `DeriveOpenBarRequest` struct.
2. Replace `pub async fn derive_open_bar` with:

```rust
pub async fn derive_open_bar(
    gateway: &crate::MarketGateway,
    repository: &dyn CanonicalKlineRepository,
    ctx: &pa_instrument::InstrumentMarketDataContext,
    timeframe: Timeframe,
) -> Result<Option<DerivedOpenBar>, AppError> {
    let session_profile = MarketSessionProfile::from_market_record(&ctx.market);
    let routed_tick = gateway.fetch_latest_tick(ctx).await?;
    let Some(bucket) =
        session_profile.current_bucket_for_tick(timeframe, routed_tick.tick.tick_time)?
    else {
        return Ok(None);
    };
    let source_timeframe = source_timeframe_for_open_bar(timeframe);
    let source_duration = duration_from_timeframe(source_timeframe)?;
    let bucket_end_open_time = bucket
        .close_time
        .checked_sub_signed(source_duration)
        .ok_or_else(|| AppError::Validation {
            message: format!(
                "failed to compute bucket end open time for {} {}",
                timeframe,
                bucket.close_time.to_rfc3339()
            ),
            source: None,
        })?;
    let bucket_rows = repository
        .list_canonical_klines(CanonicalKlineQuery {
            instrument_id: ctx.instrument.id,
            timeframe: source_timeframe,
            start_open_time: Some(bucket.open_time),
            end_open_time: Some(bucket_end_open_time),
            limit: session_profile.expected_child_bar_count(source_timeframe, timeframe)?,
            descending: false,
        })
        .await?
        .into_iter()
        .filter(|row| session_profile.accepts_bar_open(source_timeframe, row.open_time))
        .filter(|row| row.close_time <= routed_tick.tick.tick_time)
        .collect::<Vec<_>>();

    if let Some(last_row) = bucket_rows.last()
        && last_row.close_time > routed_tick.tick.tick_time
    {
        return Err(AppError::Validation {
            message: format!(
                "latest tick {} is older than latest closed child bar {}",
                routed_tick.tick.tick_time.to_rfc3339(),
                last_row.close_time.to_rfc3339()
            ),
            source: None,
        });
    }

    let (open, mut high, mut low, child_bar_count) = if let Some(first_row) = bucket_rows.first() {
        (
            first_row.open,
            bucket_rows
                .iter()
                .map(|row| row.high)
                .max()
                .expect("bucket rows should not be empty"),
            bucket_rows
                .iter()
                .map(|row| row.low)
                .min()
                .expect("bucket rows should not be empty"),
            bucket_rows.len(),
        )
    } else if let Some(previous_row) = latest_closed_row_before(
        repository,
        ctx.instrument.id,
        source_timeframe,
        bucket.open_time,
        &session_profile,
    )
    .await?
    {
        (
            previous_row.close,
            previous_row.close,
            previous_row.close,
            0,
        )
    } else {
        (
            routed_tick.tick.price,
            routed_tick.tick.price,
            routed_tick.tick.price,
            0,
        )
    };

    high = high.max(routed_tick.tick.price);
    low = low.min(routed_tick.tick.price);

    Ok(Some(DerivedOpenBar {
        instrument_id: ctx.instrument.id,
        source_timeframe,
        timeframe,
        open_time: bucket.open_time,
        close_time: bucket.close_time,
        latest_tick_time: routed_tick.tick.tick_time,
        open,
        high,
        low,
        close: routed_tick.tick.price,
        child_bar_count,
        source_provider: routed_tick.provider_name,
    }))
}
```

- [ ] **Step 2: Update lib.rs re-export**

In `crates/pa-market/src/lib.rs`, remove `DeriveOpenBarRequest` from the `pub use service::` list:

```rust
pub use service::{
    AggregateCanonicalKlinesRequest, AggregatedKline,
    DerivedOpenBar, aggregate_canonical_klines, aggregate_replay_window_rows,
    backfill_canonical_klines, derive_open_bar, list_canonical_klines,
};
```

- [ ] **Step 3: Update `tests/open_bar_runtime.rs`**

Read the current file first to understand the test structure, then update each call site.

Read: `crates/pa-market/tests/open_bar_runtime.rs`

Each test currently calls `derive_open_bar(&router, &repository, DeriveOpenBarRequest { ... })`. Replace with `derive_open_bar(&gateway, &repository, &ctx, timeframe)`.

For each test, replace the `ProviderRouter` setup with a `MarketGateway` setup (mirror Task 6 Step 3 helper functions). Replace the `DeriveOpenBarRequest` struct construction with a single `derive_open_bar` call passing `&gateway`, `&repository`, `&ctx`, and the timeframe.

Add this helper near the top of the test file (after the `StubProvider` impl):

```rust
fn ctx_for_test(market_code: &str, market_timezone: &str) -> InstrumentMarketDataContext {
    InstrumentMarketDataContext::fixture(
        market_code,
        market_timezone,
        "000001",
        "primary",
        Some("fallback"),
        "primary",
        Some("fallback"),
    )
}
```

For the test cases that previously passed `market_code: Some("cn-a")`, use `ctx_for_test("cn-a", "Asia/Shanghai")`. For `market_code: None`, use `ctx_for_test("continuous-utc", "UTC")`.

Add `use pa_instrument::InstrumentMarketDataContext;` and `MarketGateway` to the existing imports.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p pa-market --test open_bar_runtime`
Expected: all tests pass with the same assertions.

- [ ] **Step 5: Commit**

```bash
git add crates/pa-market/src/service.rs crates/pa-market/src/lib.rs crates/pa-market/tests/open_bar_runtime.rs
git commit -m "refactor(pa-market): derive_open_bar takes MarketGateway + context"
```

---

## Task 8: Reshape `aggregate_canonical_klines` + `aggregate_replay_window_rows` and update tests

**Files:**
- Modify: `crates/pa-market/src/service.rs`
- Modify: `crates/pa-market/tests/session_aggregation.rs`

- [ ] **Step 1: Shrink `AggregateCanonicalKlinesRequest`**

In `crates/pa-market/src/service.rs`, replace the `AggregateCanonicalKlinesRequest` struct definition with:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct AggregateCanonicalKlinesRequest {
    pub source_timeframe: Timeframe,
    pub target_timeframe: Timeframe,
    pub start_open_time: Option<DateTime<Utc>>,
    pub end_open_time: Option<DateTime<Utc>>,
    pub limit: usize,
}
```

- [ ] **Step 2: Update `aggregate_canonical_klines`**

Replace the function signature and body in `crates/pa-market/src/service.rs`:

```rust
pub async fn aggregate_canonical_klines(
    repository: &dyn CanonicalKlineRepository,
    ctx: &pa_instrument::InstrumentMarketDataContext,
    request: AggregateCanonicalKlinesRequest,
) -> Result<Vec<AggregatedKline>, AppError> {
    let session_profile = MarketSessionProfile::from_market_record(&ctx.market);
    let source_rows = repository
        .list_canonical_klines(CanonicalKlineQuery {
            instrument_id: ctx.instrument.id,
            timeframe: request.source_timeframe,
            start_open_time: request.start_open_time,
            end_open_time: request.end_open_time,
            limit: request.limit.saturating_mul(
                session_profile.expected_child_bar_count(
                    request.source_timeframe,
                    request.target_timeframe,
                )?,
            ),
            descending: true,
        })
        .await?;
    let mut source_rows = source_rows
        .into_iter()
        .filter(|row| session_profile.accepts_bar_open(request.source_timeframe, row.open_time))
        .collect::<Vec<_>>();
    source_rows.reverse();

    aggregate_rows(
        &source_rows,
        ctx.instrument.id,
        request.source_timeframe,
        request.target_timeframe,
        &session_profile,
    )
}
```

- [ ] **Step 3: Update `aggregate_replay_window_rows`**

Replace its signature and body:

```rust
pub fn aggregate_replay_window_rows(
    rows: &[CanonicalKlineRow],
    ctx: &pa_instrument::InstrumentMarketDataContext,
    source_timeframe: Timeframe,
    target_timeframe: Timeframe,
) -> Result<Vec<AggregatedKline>, AppError> {
    let session_profile = MarketSessionProfile::from_market_record(&ctx.market);
    let mut source_rows = rows
        .iter()
        .filter(|row| row.instrument_id == ctx.instrument.id)
        .filter(|row| row.timeframe == source_timeframe)
        .filter(|row| session_profile.accepts_bar_open(source_timeframe, row.open_time))
        .cloned()
        .collect::<Vec<_>>();
    source_rows.sort_by_key(|row| row.open_time);
    for duplicate_pair in source_rows.windows(2) {
        if duplicate_pair[0].open_time == duplicate_pair[1].open_time {
            return Err(AppError::Validation {
                message: format!(
                    "duplicate child row open_time={} for instrument {} timeframe {}",
                    duplicate_pair[0].open_time.to_rfc3339(),
                    ctx.instrument.id,
                    source_timeframe,
                ),
                source: None,
            });
        }
    }

    aggregate_rows(
        &source_rows,
        ctx.instrument.id,
        source_timeframe,
        target_timeframe,
        &session_profile,
    )
}
```

- [ ] **Step 4: Update `tests/session_aggregation.rs`**

Read: `crates/pa-market/tests/session_aggregation.rs`

For each of the 3 `aggregate_canonical_klines` call sites:
- Construct `let ctx = InstrumentMarketDataContext::fixture(<market_code>, <market_timezone>, "000001", "primary", None, "primary", None);` matching the previous `market_code` / `market_timezone` strings.
- Replace the request struct construction (which had `instrument_id`, `market_code`, `market_timezone`) with the new shrunk struct (only the 5 fields).
- Pass `&ctx` as the second argument: `aggregate_canonical_klines(&repository, &ctx, request)`.
- Where the old test populated rows with a specific `instrument_id`, replace that literal with `ctx.instrument.id`.

Add imports at the top:

```rust
use pa_instrument::InstrumentMarketDataContext;
```

If any test calls `aggregate_replay_window_rows`, apply the same translation: drop `market_code` / `market_timezone` string args, pass `&ctx`, and use `ctx.instrument.id` everywhere a hardcoded UUID was previously paired with the rows.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p pa-market --test session_aggregation`
Expected: all tests pass.

- [ ] **Step 6: Run all `pa-market` tests to verify nothing else broke**

Run: `cargo test -p pa-market`
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add crates/pa-market/src/service.rs crates/pa-market/tests/session_aggregation.rs
git commit -m "refactor(pa-market): aggregate_* take context, drop instrument/market string args"
```

---

## Task 9: Wire `MarketGateway` into `pa-api`'s `MarketRuntime`

**Files:**
- Modify: `crates/pa-api/src/router.rs`

- [ ] **Step 1: Read current `MarketRuntime`**

Read: `crates/pa-api/src/router.rs`

Locate the `MarketRuntime` struct (around lines 11–27 based on prior survey).

- [ ] **Step 2: Replace `provider_router` field with `market_gateway`**

Replace `pub provider_router: Arc<ProviderRouter>,` with `pub market_gateway: Arc<MarketGateway>,`.

Update the `MarketRuntime::new` constructor signature accordingly:

```rust
impl MarketRuntime {
    pub fn new(
        instrument_repository: InstrumentRepository,
        canonical_kline_repository: Arc<dyn CanonicalKlineRepository>,
        market_gateway: Arc<MarketGateway>,
    ) -> Self {
        Self {
            instrument_repository,
            canonical_kline_repository,
            market_gateway,
        }
    }
}
```

Update the `use pa_market::...` import at the top of the file: replace `ProviderRouter` with `MarketGateway`.

- [ ] **Step 3: Verify the crate now fails to compile (call sites still use old field)**

Run: `cargo check -p pa-api`
Expected: compile errors in `admin.rs`, `market.rs`, `analysis_runtime.rs` complaining about missing `provider_router` field. This is the signal to proceed to Tasks 10–12.

(No commit yet — tasks 9–12 land together.)

---

## Task 10: Update `pa-api/src/admin.rs`

**Files:**
- Modify: `crates/pa-api/src/admin.rs`

- [ ] **Step 1: Replace the entire `backfill_market_data` body**

In `crates/pa-api/src/admin.rs`, replace:

```rust
use pa_market::{BackfillCanonicalKlinesRequest, backfill_canonical_klines};
```

with:

```rust
use pa_market::backfill_canonical_klines;
```

Replace the body of `backfill_market_data` (the call into `backfill_canonical_klines` and its preceding manual binding lookup) with:

```rust
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
    let limit = request.limit.unwrap_or(200);

    backfill_canonical_klines(
        runtime.market_gateway.as_ref(),
        runtime.canonical_kline_repository.as_ref(),
        &context,
        timeframe,
        limit,
    )
    .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "status": "accepted",
            "instrument_id": context.instrument.id,
            "timeframe": timeframe.as_str(),
            "primary_provider": context.policy.kline_primary,
            "fallback_provider": context.policy.kline_fallback,
            "limit": limit,
        })),
    ))
}
```

- [ ] **Step 2: Verify admin.rs compiles**

Run: `cargo check -p pa-api`
Expected: errors only remain in `market.rs` and `analysis_runtime.rs`.

(No commit yet.)

---

## Task 11: Update `pa-api/src/market.rs`

**Files:**
- Modify: `crates/pa-api/src/market.rs`

- [ ] **Step 1: Read the file**

Read: `crates/pa-api/src/market.rs`

Identify the four sites (per prior grep):
- Line ~93: `aggregate_canonical_klines(...)`
- Line ~174: `fetch_latest_tick_with_fallback_source(...)`
- Line ~212: `derive_open_bar(...)`
- Plus any `resolve_market_data_context` consumer that currently unpacks fields.

- [ ] **Step 2: Update imports**

Replace any `use pa_market::{... AggregateCanonicalKlinesRequest, ProviderRouter, derive_open_bar, ...}` so that the import set matches the new shape. Remove any `BackfillCanonicalKlinesRequest`/`DeriveOpenBarRequest` imports. Add `MarketGateway` if it appears in field types (it does not in handlers, only in `MarketRuntime`).

- [ ] **Step 3: Update each `aggregate_canonical_klines` call site**

For each call, the new shape is:

```rust
let rows = aggregate_canonical_klines(
    runtime.canonical_kline_repository.as_ref(),
    &context,
    AggregateCanonicalKlinesRequest {
        source_timeframe,
        target_timeframe,
        start_open_time,
        end_open_time,
        limit,
    },
)
.await?;
```

Where `context` is the result of the (already-existing) `resolve_market_data_context` call earlier in the handler. Drop the manual `instrument_id`, `market_code`, `market_timezone` field assignments from the request struct.

- [ ] **Step 4: Update each `derive_open_bar` call site**

For each call, the new shape is:

```rust
let row = derive_open_bar(
    runtime.market_gateway.as_ref(),
    runtime.canonical_kline_repository.as_ref(),
    &context,
    timeframe,
)
.await?;
```

Drop the `DeriveOpenBarRequest` struct construction entirely.

- [ ] **Step 5: Update the direct `fetch_latest_tick_with_fallback_source` call**

The handler around line 174 currently calls `runtime.provider_router.fetch_latest_tick_with_fallback_source(...)` with manually unpacked policy/binding strings. Replace with:

```rust
let routed_tick = runtime.market_gateway.fetch_latest_tick(&context).await?;
```

Drop the manual primary/fallback provider name and binding lookups that fed the old call.

- [ ] **Step 6: Verify market.rs compiles**

Run: `cargo check -p pa-api`
Expected: errors only remain in `analysis_runtime.rs`.

(No commit yet.)

---

## Task 12: Update `pa-api/src/analysis_runtime.rs`

**Files:**
- Modify: `crates/pa-api/src/analysis_runtime.rs`

- [ ] **Step 1: Read the file and locate all six service-call sites**

Read: `crates/pa-api/src/analysis_runtime.rs`

Per prior grep, the sites are:
- Line ~534: `aggregate_canonical_klines(...)`
- Line ~578: `derive_open_bar(...)`
- Line ~677: `aggregate_canonical_klines(...)`
- Line ~720: `derive_open_bar(...)`
- Line ~754: `fetch_latest_tick_with_fallback_source(...)` (on a `ProviderRouter`)
- Line ~788: `derive_open_bar(...)`

- [ ] **Step 2: Update imports**

Replace `use pa_market::{... aggregate_canonical_klines, derive_open_bar, ...}` so it no longer references `DeriveOpenBarRequest`. Keep `AggregateCanonicalKlinesRequest`. Add `MarketGateway` only if a field in this file uses it (typically not; the runtime is reached through `state.market_runtime`).

If this file has any helper functions that currently take `provider_router: &ProviderRouter`, change them to take `market_gateway: &MarketGateway`. The helper signatures around lines 454–1003 (per prior grep) take `&InstrumentMarketDataContext` but may also pass `provider_router` separately — adjust accordingly.

- [ ] **Step 3: Update each `aggregate_canonical_klines` call site (lines ~534, ~677)**

For each call, find the preceding `let context = ...resolve_market_data_context(...)` and reuse it. Replace the call with:

```rust
let rows = aggregate_canonical_klines(
    runtime.canonical_kline_repository.as_ref(),
    &context,
    AggregateCanonicalKlinesRequest {
        source_timeframe,
        target_timeframe,
        start_open_time,
        end_open_time,
        limit,
    },
)
.await?;
```

Drop the previous `instrument_id`, `market_code`, `market_timezone` fields from the request struct construction.

- [ ] **Step 4: Update each `derive_open_bar` call site (lines ~578, ~720, ~788)**

For each call, replace with:

```rust
let row = derive_open_bar(
    runtime.market_gateway.as_ref(),
    runtime.canonical_kline_repository.as_ref(),
    &context,
    timeframe,
)
.await?;
```

Drop the `DeriveOpenBarRequest` struct construction entirely. Reuse the `context` variable from the preceding `resolve_market_data_context` call. If a helper function in this file currently takes `provider_router: &ProviderRouter` and threads it down to `derive_open_bar`, change its signature to `market_gateway: &MarketGateway` and pass `runtime.market_gateway.as_ref()` at the top-level caller.

- [ ] **Step 5: Update the raw `fetch_latest_tick_with_fallback_source` call (line ~754)**

The call is currently:

```rust
runtime.provider_router.fetch_latest_tick_with_fallback_source(
    primary_provider,
    fallback_provider,
    &primary_binding.provider_symbol,
    &fallback_binding.provider_symbol,
).await?
```

Replace with:

```rust
runtime.market_gateway.fetch_latest_tick(&context).await?
```

Delete the lines that extracted `primary_provider`, `fallback_provider`, `primary_binding`, `fallback_binding` from the context — they are no longer needed at this call site.

- [ ] **Step 6: Verify the whole crate compiles**

Run: `cargo check -p pa-api`
Expected: clean build.

(No commit yet — Task 13 wraps the wiring.)

---

## Task 13: Wire `MarketGateway` in `pa-app/src/main.rs` and verify

**Files:**
- Modify: `crates/pa-app/src/main.rs`

- [ ] **Step 1: Read the current main.rs**

Read: `crates/pa-app/src/main.rs`

Locate the `let mut provider_router = ProviderRouter::default();` block (around line 54) and the subsequent `MarketRuntime::new(...)` call.

- [ ] **Step 2: Wrap the provider router into a `MarketGateway`**

Replace the construction of `MarketRuntime` so that the `provider_router` is wrapped:

```rust
use pa_market::MarketGateway;

// after providers are inserted into provider_router:
let market_gateway = Arc::new(MarketGateway::new(provider_router));

let market_runtime = Arc::new(MarketRuntime::new(
    instrument_repository.clone(),
    canonical_kline_repository.clone(),
    market_gateway,
));
```

If `provider_router` is used elsewhere in `main.rs` after this point (e.g., handed to `replay_live`-related code), thread the gateway and use `gateway.router()` at the boundary.

- [ ] **Step 3: Update `replay_live.rs` if it consumes runtime via the old field**

Read: `crates/pa-app/src/replay_live.rs`

`replay_live.rs` operates at the lower abstraction level (raw provider calls without context resolution). It continues to take `&ProviderRouter`. If `main.rs` previously passed `&provider_router` to a `replay_live` entry point, change the call site to pass `gateway.router()` (since the gateway now owns the router):

```rust
some_replay_live_fn(gateway.router(), /* other args */).await?;
```

No changes needed inside `replay_live.rs` itself — its function signatures stay as-is.

- [ ] **Step 4: Verify the full workspace compiles**

Run: `cargo build --workspace`
Expected: clean.

- [ ] **Step 5: Run the full test suite**

Run: `cargo test --workspace`
Expected: all tests pass (existing + new gateway tests).

- [ ] **Step 6: Commit the wiring + all dependent call-site updates together**

```bash
git add crates/pa-api/src/router.rs crates/pa-api/src/admin.rs crates/pa-api/src/market.rs crates/pa-api/src/analysis_runtime.rs crates/pa-app/src/main.rs
git commit -m "refactor(pa-api,pa-app): consume MarketGateway through MarketRuntime"
```

---

## Task 14: Verify Definition of Done

**Files:** (no changes — verification only)

- [ ] **Step 1: Confirm DoD §8.3 — request structs no longer carry routing strings**

Run: `grep -nE "primary_provider|fallback_provider|primary_provider_symbol|fallback_provider_symbol|market_code|market_timezone" crates/pa-market/src/service.rs`
Expected: no struct field hits (matches inside doc comments or local variable names are fine).

If this returns anything, treat it as a Task 6/7/8 regression and fix before proceeding.

- [ ] **Step 2: Confirm DoD §8.4 — business code free of old provider-symbol args**

Run: `grep -rn "primary_provider_symbol\|fallback_provider_symbol" crates/`
Expected: zero results in non-test, non-fixture code.

If hits remain inside `crates/*/tests/` it's acceptable only if the file is a fixture builder; otherwise add a fix-up commit.

- [ ] **Step 3: Run smoke tests**

Run: `cargo test -p pa-api --test smoke -p pa-app --test live_replay`
Expected: PASS.

- [ ] **Step 4: Final full test suite**

Run: `cargo test --workspace`
Expected: all green.

(No commit.)

---

## Task 15: Update observability foundation spec catalog (DoD §8.5)

**Files:**
- Modify: `docs/superpowers/specs/2026-04-26-observability-foundation-design.md`

- [ ] **Step 1: Add `market` label to the orchestration metrics table**

In §6.1 Orchestration Domain, edit the rows for `orchestration_tasks_total` and `orchestration_task_duration_seconds`:

```markdown
| `orchestration_tasks_total` | counter | `status`, `prompt_key`, `market` | Task state transitions |
| `orchestration_task_duration_seconds` | histogram | `prompt_key`, `outcome`, `market` | End-to-end task time |
```

- [ ] **Step 2: Add `market` label to LLM domain**

In §6.2, edit `llm_request_duration_seconds`:

```markdown
| `llm_request_duration_seconds` | histogram | `provider`, `model`, `outcome`, `market` | Per-call latency |
```

- [ ] **Step 3: Add `market` label to Market domain**

In §6.3, edit:

```markdown
| `market_provider_requests_total` | counter | `provider`, `outcome`, `market` | Provider call volume |
| `market_provider_duration_seconds` | histogram | `provider`, `market` | Provider latency |
```

(`market_bars_ingested_total` already has a `market` label — no change.)

- [ ] **Step 4: Add a paragraph about the `market` label semantics**

Add a new subsection after §6.5, before §6.6:

```markdown
### 6.5.1 The `market` Label

All metrics that carry a `market` label source the value from
`InstrumentMarketDataContext::market.code`, populated by the
`MarketGateway` introduced in sub-project B
(`docs/superpowers/specs/2026-04-26-market-gateway-design.md`).

Instrumentation points that genuinely have no instrument context
(infrastructure metrics, OTLP exporter metrics, LLM calls dispatched
without an instrument) emit the literal value `unknown`. This is a
documented escape hatch, not a workaround for missing plumbing — the CI
catalog check (§7.3) enforces that any newly-added metric carrying the
`market` label appears in `signals.md` with `unknown` documented as a
permitted value.

The label uses the immutable, controlled vocabulary `Market.code`
(e.g., `cn-a`, `continuous-utc`, `fx`), so cardinality is bounded by
the number of supported markets — currently a handful, never user
input.
```

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-04-26-observability-foundation-design.md
git commit -m "docs(observability): add market label to catalog (sub-project B prep)"
```

---

## Spec Coverage Self-Review (already performed)

The plan covers each spec section as follows:

| Spec section | Plan task(s) |
|---|---|
| §5.1 Crate Layering (`pa-market → pa-instrument`) | Task 1 |
| §5.2 `MarketGateway` API | Tasks 3, 4 |
| §5.3 Error semantics | Task 4 (validation tests) |
| §5.4 Workflow function reshape | Tasks 6, 7, 8 |
| §5.5 `from_market_record` | Task 5 |
| §5.6 Fixture helper | Task 1 |
| §6 Migration plan | Tasks 9, 10, 11, 12, 13 |
| §7.1 Gateway unit tests (5 cases) | Tasks 3 (1 test), 4 (4 tests) |
| §7.2 Existing workflow tests updated | Tasks 6, 7, 8 |
| §8 DoD verification | Task 14 |
| §8.5 Observability spec catalog edit | Task 15 |
