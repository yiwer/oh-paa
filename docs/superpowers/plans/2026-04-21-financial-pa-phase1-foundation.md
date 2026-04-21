# Financial PA Phase 1 Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Phase 1 data-first foundation for a multi-market price action analysis server with canonical K-lines, primary/fallback providers, shared per-bar analysis, shared daily market context, and minimal user-triggered analysis.

**Architecture:** Convert the current single-crate skeleton into a Rust workspace with focused crates that mirror the approved design: shared core types, instrument registry, market-data SPI plus canonicalization, shared analysis, user flows, HTTP API, and an app crate for wiring. Implement the system as a vertical slice: schema and registry first, then provider abstraction, then canonical market data, then shared analysis, then minimal user analysis, then APIs and observability.

**Tech Stack:** Rust 2024, Tokio, Axum, SQLx + PostgreSQL, Serde, Reqwest, Tracing, Chrono, UUID, Rust Decimal, Async Trait.

---

## File Structure Map

Create or reshape the repository into this layout before feature work:

- `Cargo.toml`
  - Workspace root defining shared dependencies and crate members.
- `crates/pa-core/src/lib.rs`
  - Re-export shared config, error, timeframe, market, and task-state primitives.
- `crates/pa-core/src/config.rs`
  - Runtime config structs and environment loading.
- `crates/pa-core/src/error.rs`
  - Shared application error type plus provider and validation error helpers.
- `crates/pa-core/src/timeframe.rs`
  - `Timeframe` enum and bar-duration helpers for `15m`, `1h`, `1d`.
- `crates/pa-instrument/src/lib.rs`
  - Instrument registry public surface.
- `crates/pa-instrument/src/models.rs`
  - Market, instrument, symbol-binding, provider-policy domain models.
- `crates/pa-instrument/src/repository.rs`
  - SQLx persistence for markets, instruments, symbol bindings, and provider policy.
- `crates/pa-instrument/src/service.rs`
  - Provider-policy resolution and registry orchestration.
- `crates/pa-market/src/lib.rs`
  - Market-data public surface.
- `crates/pa-market/src/provider.rs`
  - Provider SPI traits and routing contract.
- `crates/pa-market/src/models.rs`
  - Canonical tick, canonical K-line, and open-bar domain types.
- `crates/pa-market/src/normalize.rs`
  - Provider payload to canonical data normalization and validation.
- `crates/pa-market/src/open_bar.rs`
  - In-memory open-bar derivation logic.
- `crates/pa-market/src/repository.rs`
  - SQLx persistence for canonical K-lines and tick snapshots.
- `crates/pa-market/src/service.rs`
  - Backfill, latest-tick ingestion, and provider fallback orchestration.
- `crates/pa-market/src/providers/eastmoney.rs`
  - EastMoney provider adapter.
- `crates/pa-market/src/providers/twelvedata.rs`
  - TwelveData provider adapter.
- `crates/pa-analysis/src/lib.rs`
  - Shared analysis public surface.
- `crates/pa-analysis/src/models.rs`
  - `bar_analysis`, `daily_market_context`, and analysis task models.
- `crates/pa-analysis/src/repository.rs`
  - SQLx persistence for shared analysis outputs and task state.
- `crates/pa-analysis/src/service.rs`
  - Bar-analysis and daily-context orchestration.
- `crates/pa-analysis/src/bar_worker.rs`
  - Handler for closed-bar-triggered public analysis.
- `crates/pa-analysis/src/daily_context_worker.rs`
  - Handler for daily market-context generation.
- `crates/pa-user/src/lib.rs`
  - User subscription and user-analysis public surface.
- `crates/pa-user/src/models.rs`
  - Subscription, position, user-analysis task, and user-analysis report models.
- `crates/pa-user/src/repository.rs`
  - SQLx persistence for subscriptions and positions.
- `crates/pa-user/src/service.rs`
  - User-level orchestration using shared analysis plus positions.
- `crates/pa-api/src/lib.rs`
  - Router factory.
- `crates/pa-api/src/admin.rs`
  - Admin endpoints for markets, instruments, policy, backfill, and analysis re-run.
- `crates/pa-api/src/market.rs`
  - Canonical K-line, latest tick, and open-bar endpoints.
- `crates/pa-api/src/analysis.rs`
  - Shared analysis endpoints.
- `crates/pa-api/src/user.rs`
  - Subscription, position, and manual user-analysis endpoints.
- `crates/pa-api/src/router.rs`
  - Route composition and shared state.
- `crates/pa-app/src/main.rs`
  - Config loading, database pool creation, provider registry setup, scheduler wiring, and server start.
- `migrations/001_initial.sql`
  - Phase 1 schema for registry, market data, shared analysis, and user-side minimum tables.
- `docs/architecture/phase1-runtime.md`
  - Short operator-facing runtime notes once the app wiring is complete.

## Decomposition Note

The approved spec spans several subsystems, but they are tightly coupled by the `instrument_id -> provider policy -> canonical K-line -> shared analysis -> user analysis` pipeline. Instead of creating four separate plans, this document keeps one plan with narrow vertical tasks that each end in a working, testable increment.

### Task 1: Bootstrap the Workspace and Shared Core Primitives

**Files:**
- Modify: `E:\rust-app\oh-paa\Cargo.toml`
- Delete: `E:\rust-app\oh-paa\src\main.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-core\Cargo.toml`
- Create: `E:\rust-app\oh-paa\crates\pa-core\src\lib.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-core\src\config.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-core\src\error.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-core\src\timeframe.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-core\tests\timeframe.rs`

- [ ] **Step 1: Write the failing timeframe test**

```rust
use std::time::Duration;

use pa_core::timeframe::Timeframe;

#[test]
fn timeframe_duration_matches_phase1_contract() {
    assert_eq!(Timeframe::M15.as_str(), "15m");
    assert_eq!(Timeframe::H1.as_str(), "1h");
    assert_eq!(Timeframe::D1.as_str(), "1d");

    assert_eq!(Timeframe::M15.duration(), Duration::from_secs(15 * 60));
    assert_eq!(Timeframe::H1.duration(), Duration::from_secs(60 * 60));
    assert_eq!(Timeframe::D1.duration(), Duration::from_secs(24 * 60 * 60));
}
```

- [ ] **Step 2: Run the test to verify it fails because the workspace and crate do not exist yet**

Run: `cargo test -p pa-core timeframe_duration_matches_phase1_contract -- --exact`

Expected: FAIL with an error like `package ID specification 'pa-core' did not match any packages`

- [ ] **Step 3: Replace the root crate with a workspace and add the `pa-core` crate**

```toml
[workspace]
resolver = "3"
members = ["crates/pa-core"]

[workspace.dependencies]
anyhow = "1"
async-trait = "0.1"
axum = { version = "0.8", features = ["macros"] }
chrono = { version = "0.4", features = ["serde"] }
reqwest = { version = "0.12", features = ["json"] }
rust_decimal = { version = "1", features = ["serde-with-str", "db-postgres"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "postgres", "uuid", "chrono", "rust_decimal", "migrate"] }
thiserror = "2"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal"] }
toml = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
uuid = { version = "1", features = ["serde", "v4"] }
```

```rust
// crates/pa-core/src/lib.rs
pub mod config;
pub mod error;
pub mod timeframe;
```

```rust
// crates/pa-core/src/timeframe.rs
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timeframe {
    M15,
    H1,
    D1,
}

impl Timeframe {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::M15 => "15m",
            Self::H1 => "1h",
            Self::D1 => "1d",
        }
    }

    pub fn duration(self) -> Duration {
        match self {
            Self::M15 => Duration::from_secs(15 * 60),
            Self::H1 => Duration::from_secs(60 * 60),
            Self::D1 => Duration::from_secs(24 * 60 * 60),
        }
    }
}
```

- [ ] **Step 4: Run the test again**

Run: `cargo test -p pa-core timeframe_duration_matches_phase1_contract -- --exact`

Expected: PASS with one passing test

- [ ] **Step 5: Add shared config and error primitives needed by later crates**

```rust
// crates/pa-core/src/config.rs
use serde::Deserialize;

use crate::error::AppError;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_addr: String,
    pub eastmoney_base_url: Option<String>,
    pub twelvedata_base_url: Option<String>,
    pub twelvedata_api_key: Option<String>,
}

pub fn load() -> Result<AppConfig, AppError> {
    let raw = std::fs::read_to_string("config.toml")
        .map_err(|err| AppError::Storage(err.to_string()))?;
    toml::from_str(&raw).map_err(|err| AppError::Validation(err.to_string()))
}
```

```rust
// crates/pa-core/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("validation error: {0}")]
    Validation(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("analysis error: {0}")]
    Analysis(String),
}
```

- [ ] **Step 6: Commit the workspace bootstrap**

```bash
git add Cargo.toml crates/pa-core src
git commit -m "chore: bootstrap workspace and core primitives"
```

- [ ] **Step 7: Expand the workspace member list now that Task 2 is ready to add the next crate**

```toml
[workspace]
resolver = "3"
members = [
    "crates/pa-core",
    "crates/pa-instrument",
]
```

### Task 2: Add the Phase 1 Schema and Instrument Registry

**Files:**
- Create: `E:\rust-app\oh-paa\migrations\001_initial.sql`
- Create: `E:\rust-app\oh-paa\crates\pa-instrument\Cargo.toml`
- Create: `E:\rust-app\oh-paa\crates\pa-instrument\src\lib.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-instrument\src\models.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-instrument\src\repository.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-instrument\src\service.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-instrument\tests\provider_policy.rs`

- [ ] **Step 1: Write the failing provider-policy resolution test**

```rust
use pa_instrument::models::{PolicyScope, ProviderPolicy};
use pa_instrument::service::resolve_policy;

#[test]
fn instrument_override_wins_over_market_default() {
    let market = ProviderPolicy::new(
        PolicyScope::Market("crypto".into()),
        "twelvedata",
        Some("eastmoney"),
        "twelvedata",
        None,
    );
    let instrument = ProviderPolicy::new(
        PolicyScope::Instrument("btc-usdt".into()),
        "eastmoney",
        Some("twelvedata"),
        "twelvedata",
        None,
    );

    let resolved = resolve_policy(Some(&instrument), Some(&market)).unwrap();

    assert_eq!(resolved.kline_primary, "eastmoney");
    assert_eq!(resolved.kline_fallback.as_deref(), Some("twelvedata"));
}
```

- [ ] **Step 2: Run the test to verify it fails because `pa-instrument` does not exist yet**

Run: `cargo test -p pa-instrument instrument_override_wins_over_market_default -- --exact`

Expected: FAIL with a missing package or missing crate error

- [ ] **Step 3: Create the initial schema for registry, market data, analysis, and user minimum tables**

```sql
CREATE TABLE markets (
    market_code TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    timezone TEXT NOT NULL,
    session_template JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE instruments (
    instrument_id UUID PRIMARY KEY,
    market_code TEXT NOT NULL REFERENCES markets(market_code),
    display_name TEXT NOT NULL,
    base_currency TEXT NOT NULL,
    quote_currency TEXT NOT NULL,
    trading_status TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE instrument_symbol_bindings (
    instrument_id UUID NOT NULL REFERENCES instruments(instrument_id),
    provider TEXT NOT NULL,
    provider_symbol TEXT NOT NULL,
    provider_exchange TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (instrument_id, provider, provider_symbol)
);

CREATE TABLE provider_policies (
    scope_type TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    kline_primary TEXT NOT NULL,
    kline_fallback TEXT,
    tick_primary TEXT NOT NULL,
    tick_fallback TEXT,
    PRIMARY KEY (scope_type, scope_id)
);

CREATE TABLE canonical_klines (
    instrument_id UUID NOT NULL REFERENCES instruments(instrument_id),
    timeframe TEXT NOT NULL,
    bar_open_time TIMESTAMPTZ NOT NULL,
    bar_close_time TIMESTAMPTZ NOT NULL,
    open NUMERIC NOT NULL,
    high NUMERIC NOT NULL,
    low NUMERIC NOT NULL,
    close NUMERIC NOT NULL,
    volume NUMERIC,
    source_provider TEXT NOT NULL,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (instrument_id, timeframe, bar_open_time)
);
```

- [ ] **Step 4: Implement the instrument models and policy-resolution service**

```rust
// crates/pa-instrument/src/models.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyScope {
    Market(String),
    Instrument(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPolicy {
    pub scope: PolicyScope,
    pub kline_primary: String,
    pub kline_fallback: Option<String>,
    pub tick_primary: String,
    pub tick_fallback: Option<String>,
}

impl ProviderPolicy {
    pub fn new(
        scope: PolicyScope,
        kline_primary: &str,
        kline_fallback: Option<&str>,
        tick_primary: &str,
        tick_fallback: Option<&str>,
    ) -> Self {
        Self {
            scope,
            kline_primary: kline_primary.into(),
            kline_fallback: kline_fallback.map(str::to_owned),
            tick_primary: tick_primary.into(),
            tick_fallback: tick_fallback.map(str::to_owned),
        }
    }
}
```

```rust
// crates/pa-instrument/src/service.rs
use pa_core::error::AppError;

use crate::models::ProviderPolicy;

pub fn resolve_policy(
    instrument_policy: Option<&ProviderPolicy>,
    market_policy: Option<&ProviderPolicy>,
) -> Result<ProviderPolicy, AppError> {
    instrument_policy
        .cloned()
        .or_else(|| market_policy.cloned())
        .ok_or_else(|| AppError::Validation("missing provider policy".into()))
}
```

- [ ] **Step 5: Run the policy test and add one SQLx repository smoke test**

Run: `cargo test -p pa-instrument instrument_override_wins_over_market_default -- --exact`

Expected: PASS

Run: `cargo test -p pa-instrument --test provider_policy`

Expected: PASS with the policy-resolution test and one repository smoke test that inserts a market and one instrument

- [ ] **Step 6: Commit the registry foundation**

```bash
git add Cargo.toml migrations crates/pa-instrument
git commit -m "feat: add phase1 schema and instrument registry"
```

### Task 3: Define the Market-Data SPI and Canonical Domain Types

**Files:**
- Create: `E:\rust-app\oh-paa\crates\pa-market\Cargo.toml`
- Create: `E:\rust-app\oh-paa\crates\pa-market\src\lib.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-market\src\provider.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-market\src\models.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-market\src\normalize.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-market\tests\normalize_kline.rs`

- [ ] **Step 1: Write the failing normalization test for invalid OHLC data**

```rust
use rust_decimal::Decimal;
use pa_market::models::ProviderKline;
use pa_market::normalize::normalize_kline;

#[test]
fn normalize_rejects_high_below_close() {
    let provider_bar = ProviderKline {
        open: Decimal::new(100, 0),
        high: Decimal::new(101, 0),
        low: Decimal::new(99, 0),
        close: Decimal::new(102, 0),
        ..ProviderKline::fixture()
    };

    let err = normalize_kline(provider_bar).unwrap_err();
    assert!(err.to_string().contains("high"));
}
```

- [ ] **Step 2: Run the test to verify it fails because the crate is not present**

Run: `cargo test -p pa-market normalize_rejects_high_below_close -- --exact`

Expected: FAIL with a missing package error

- [ ] **Step 3: Add the provider SPI and canonical domain models**

```rust
// crates/pa-market/src/provider.rs
use async_trait::async_trait;
use pa_core::{error::AppError, timeframe::Timeframe};

use crate::models::{ProviderKline, ProviderTick};

#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    fn name(&self) -> &'static str;

    async fn fetch_klines(
        &self,
        provider_symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError>;

    async fn fetch_latest_tick(
        &self,
        provider_symbol: &str,
    ) -> Result<ProviderTick, AppError>;

    async fn healthcheck(&self) -> Result<(), AppError>;
}
```

```rust
// crates/pa-market/src/models.rs
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ProviderKline {
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct ProviderTick {
    pub price: Decimal,
    pub size: Option<Decimal>,
    pub tick_time: DateTime<Utc>,
}

impl ProviderKline {
    pub fn fixture() -> Self {
        let now = Utc::now();
        Self {
            open_time: now,
            close_time: now,
            open: Decimal::ONE,
            high: Decimal::ONE,
            low: Decimal::ONE,
            close: Decimal::ONE,
            volume: Some(Decimal::ONE),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CanonicalKline {
    pub instrument_id: Uuid,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>,
    pub source_provider: String,
}
```

- [ ] **Step 4: Implement normalization and validation**

```rust
// crates/pa-market/src/normalize.rs
use pa_core::error::AppError;

use crate::models::ProviderKline;

pub fn normalize_kline(bar: ProviderKline) -> Result<ProviderKline, AppError> {
    if bar.high < bar.open || bar.high < bar.close {
        return Err(AppError::Validation(
            "high must be >= open and close".into(),
        ));
    }

    if bar.low > bar.open || bar.low > bar.close {
        return Err(AppError::Validation(
            "low must be <= open and close".into(),
        ));
    }

    if bar.close_time <= bar.open_time {
        return Err(AppError::Validation(
            "close_time must be greater than open_time".into(),
        ));
    }

    Ok(bar)
}
```

- [ ] **Step 5: Run the normalization tests**

Run: `cargo test -p pa-market normalize_rejects_high_below_close -- --exact`

Expected: PASS

- [ ] **Step 6: Commit the market-data SPI foundation**

```bash
git add crates/pa-market
git commit -m "feat: add market-data spi and canonical types"
```

- [ ] **Step 7: Expand the workspace members for the new market crate**

```toml
[workspace]
resolver = "3"
members = [
    "crates/pa-core",
    "crates/pa-instrument",
    "crates/pa-market",
]
```

### Task 4: Implement EastMoney and TwelveData Adapters with Fallback Routing

**Files:**
- Create: `E:\rust-app\oh-paa\crates\pa-market\src\providers\mod.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-market\src\providers\eastmoney.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-market\src\providers\twelvedata.rs`
- Modify: `E:\rust-app\oh-paa\crates\pa-market\src\provider.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-market\tests\provider_router.rs`

- [ ] **Step 1: Write the failing fallback test**

```rust
use std::sync::Arc;

use std::collections::HashMap;

use pa_market::provider::ProviderRouter;
use pa_core::timeframe::Timeframe;

#[tokio::test]
async fn router_uses_fallback_when_primary_fails() {
    let router = ProviderRouter::new(
        HashMap::from([
            ("primary".to_string(), Arc::new(FailingProvider::new()) as Arc<dyn pa_market::provider::MarketDataProvider>),
            ("fallback".to_string(), Arc::new(FixtureProvider::new()) as Arc<dyn pa_market::provider::MarketDataProvider>),
        ]),
    );

    let bars = router
        .fetch_klines_with_fallback("primary", Some("fallback"), "BTC/USD", Timeframe::M15, 1)
        .await
        .unwrap();

    assert_eq!(bars.len(), 1);
}
```

- [ ] **Step 2: Run the fallback test to verify it fails**

Run: `cargo test -p pa-market router_uses_fallback_when_primary_fails -- --exact`

Expected: FAIL because `ProviderRouter` is not implemented yet

- [ ] **Step 3: Implement a router that tries primary then fallback**

```rust
// crates/pa-market/src/provider.rs
pub type StaticProviderMap = std::collections::HashMap<
    String,
    std::sync::Arc<dyn MarketDataProvider>,
>;

pub struct ProviderRouter {
    providers: StaticProviderMap,
}

impl ProviderRouter {
    pub fn new(providers: StaticProviderMap) -> Self {
        Self { providers }
    }

    pub async fn fetch_klines_with_fallback(
        &self,
        primary: &str,
        fallback: Option<&str>,
        provider_symbol: &str,
        timeframe: Timeframe,
        limit: usize,
    ) -> Result<Vec<ProviderKline>, AppError> {
        let primary_provider = self
            .providers
            .get(primary)
            .ok_or_else(|| AppError::Provider(format!("unknown provider: {primary}")))?;

        match primary_provider
            .fetch_klines(provider_symbol, timeframe, limit)
            .await
        {
            Ok(result) if !result.is_empty() => Ok(result),
            Ok(_) | Err(_) => {
                let fallback = fallback.ok_or_else(|| AppError::Provider("primary failed and no fallback configured".into()))?;
                let fallback_provider = self
                    .providers
                    .get(fallback)
                    .ok_or_else(|| AppError::Provider(format!("unknown provider: {fallback}")))?;
                fallback_provider.fetch_klines(provider_symbol, timeframe, limit).await
            }
        }
    }
}
```

- [ ] **Step 4: Implement both provider adapters against their upstream payload formats**

```rust
// crates/pa-market/src/providers/eastmoney.rs
pub struct EastMoneyProvider {
    client: reqwest::Client,
    base_url: String,
}

// Parse EastMoney response fields into ProviderKline and ProviderTick.
// Keep all HTTP-specific response structs private to this module.
```

```rust
// crates/pa-market/src/providers/twelvedata.rs
pub struct TwelveDataProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

// Parse TwelveData response fields into ProviderKline and ProviderTick.
// Normalize provider-specific symbol shape before returning domain types.
```

- [ ] **Step 5: Run provider contract and fallback tests**

Run: `cargo test -p pa-market --test provider_router`

Expected: PASS with:
- one fallback-routing test
- one EastMoney payload parsing test
- one TwelveData payload parsing test

- [ ] **Step 6: Commit the provider implementations**

```bash
git add Cargo.toml crates/pa-market
git commit -m "feat: add eastmoney and twelvedata providers"
```

### Task 5: Build Canonical K-line Persistence, Tick Snapshots, and Open-Bar Derivation

**Files:**
- Create: `E:\rust-app\oh-paa\crates\pa-market\src\repository.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-market\src\open_bar.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-market\src\service.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-market\tests\open_bar.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-market\tests\backfill_idempotent.rs`

- [ ] **Step 1: Write the failing open-bar test**

```rust
use chrono::{DateTime, Utc};
use pa_market::open_bar::OpenBarBook;
use rust_decimal::Decimal;

#[test]
fn latest_tick_updates_high_low_and_close_without_touching_open() {
    let mut book = OpenBarBook::default();
    let t1 = DateTime::parse_from_rfc3339("2026-04-21T09:30:00Z").unwrap().with_timezone(&Utc);
    let t2 = DateTime::parse_from_rfc3339("2026-04-21T09:35:00Z").unwrap().with_timezone(&Utc);
    let t3 = DateTime::parse_from_rfc3339("2026-04-21T09:40:00Z").unwrap().with_timezone(&Utc);

    book.start_bar("15m", Decimal::new(100, 0), t1);
    book.apply_tick("15m", Decimal::new(103, 0), t2);
    book.apply_tick("15m", Decimal::new(99, 0), t3);

    let bar = book.current("15m").unwrap();
    assert_eq!(bar.open, Decimal::new(100, 0));
    assert_eq!(bar.high, Decimal::new(103, 0));
    assert_eq!(bar.low, Decimal::new(99, 0));
    assert_eq!(bar.close, Decimal::new(99, 0));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p pa-market latest_tick_updates_high_low_and_close_without_touching_open -- --exact`

Expected: FAIL because `OpenBarBook` is not implemented yet

- [ ] **Step 3: Implement the in-memory open-bar book**

```rust
// crates/pa-market/src/open_bar.rs
use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct OpenBarState {
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub last_tick_time: DateTime<Utc>,
}

#[derive(Default)]
pub struct OpenBarBook {
    bars: HashMap<String, OpenBarState>,
}

impl OpenBarBook {
    pub fn start_bar(&mut self, timeframe: &str, price: Decimal, tick_time: DateTime<Utc>) {
        self.bars.insert(
            timeframe.to_owned(),
            OpenBarState {
                open: price,
                high: price,
                low: price,
                close: price,
                last_tick_time: tick_time,
            },
        );
    }

    pub fn apply_tick(&mut self, timeframe: &str, price: Decimal, tick_time: DateTime<Utc>) {
        if let Some(bar) = self.bars.get_mut(timeframe) {
            if price > bar.high {
                bar.high = price;
            }
            if price < bar.low {
                bar.low = price;
            }
            bar.close = price;
            bar.last_tick_time = tick_time;
        }
    }

    pub fn current(&self, timeframe: &str) -> Option<&OpenBarState> {
        self.bars.get(timeframe)
    }
}
```

- [ ] **Step 4: Add canonical K-line upsert and backfill service logic**

```rust
// crates/pa-market/src/service.rs
pub async fn backfill_canonical_klines(
    router: &ProviderRouter,
    repo: &MarketDataRepository,
    policy: &ProviderPolicy,
    provider_symbol: &str,
    instrument_id: Uuid,
    timeframe: Timeframe,
) -> Result<(), AppError> {
    let bars = router
        .fetch_klines_with_fallback(
            &policy.kline_primary,
            policy.kline_fallback.as_deref(),
            provider_symbol,
        )
        .await?;

    for bar in bars {
        let bar = normalize_kline(bar)?;
        repo.upsert_closed_bar(instrument_id, timeframe, bar, &policy.kline_primary)
            .await?;
    }

    Ok(())
}
```

- [ ] **Step 5: Run the open-bar and idempotency tests**

Run: `cargo test -p pa-market --test open_bar`

Expected: PASS with the open-bar mutation test

Run: `cargo test -p pa-market --test backfill_idempotent`

Expected: PASS with one test proving repeated backfill leaves one canonical row per `(instrument_id, timeframe, bar_open_time)`

- [ ] **Step 6: Commit the canonical market-data pipeline**

```bash
git add crates/pa-market
git commit -m "feat: add canonical kline ingestion and open bar derivation"
```

- [ ] **Step 7: Expand the workspace members for the analysis crate**

```toml
[workspace]
resolver = "3"
members = [
    "crates/pa-core",
    "crates/pa-instrument",
    "crates/pa-market",
    "crates/pa-analysis",
]
```

### Task 6: Implement Shared Bar Analysis and Daily Market Context

**Files:**
- Create: `E:\rust-app\oh-paa\crates\pa-analysis\Cargo.toml`
- Create: `E:\rust-app\oh-paa\crates\pa-analysis\src\lib.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-analysis\src\models.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-analysis\src\repository.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-analysis\src\service.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-analysis\src\bar_worker.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-analysis\src\daily_context_worker.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-analysis\tests\bar_analysis_task.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-analysis\tests\daily_context_task.rs`

- [ ] **Step 1: Write the failing shared-analysis idempotency test**

```rust
use pa_analysis::service::SharedAnalysisService;

#[tokio::test]
async fn bar_analysis_is_unique_per_bar_and_version() {
    let service = SharedAnalysisService::new(FixtureRepo::default(), FixtureLlm::default());

    service.generate_bar_analysis(FixtureInput::btc_15m()).await.unwrap();
    service.generate_bar_analysis(FixtureInput::btc_15m()).await.unwrap();

    assert_eq!(service.repo().bar_analysis_count().await, 1);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p pa-analysis bar_analysis_is_unique_per_bar_and_version -- --exact`

Expected: FAIL because `pa-analysis` does not exist yet

- [ ] **Step 3: Add the shared-analysis models and repository contract**

```rust
// crates/pa-analysis/src/models.rs
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarAnalysis {
    pub instrument_id: Uuid,
    pub timeframe: String,
    pub bar_close_time: DateTime<Utc>,
    pub analysis_version: String,
    pub result_json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyMarketContext {
    pub instrument_id: Uuid,
    pub trading_date: NaiveDate,
    pub analysis_version: String,
    pub context_json: serde_json::Value,
}
```

- [ ] **Step 4: Implement service methods for closed-bar and daily-context generation**

```rust
// crates/pa-analysis/src/service.rs
pub async fn generate_bar_analysis(&self, input: BarAnalysisInput) -> Result<(), AppError> {
    if self.repo.exists_bar_analysis(&input.identity_key()).await? {
        return Ok(());
    }

    let output = self.llm.generate_bar_analysis(&input).await?;
    self.repo.insert_bar_analysis(output).await
}

pub async fn generate_daily_context(&self, input: DailyContextInput) -> Result<(), AppError> {
    if self.repo.exists_daily_context(&input.identity_key()).await? {
        return Ok(());
    }

    let output = self.llm.generate_daily_context(&input).await?;
    self.repo.insert_daily_context(output).await
}
```

- [ ] **Step 5: Run the shared-analysis tests**

Run: `cargo test -p pa-analysis --test bar_analysis_task`

Expected: PASS with idempotent insert behavior

Run: `cargo test -p pa-analysis --test daily_context_task`

Expected: PASS with one daily output per instrument per date

- [ ] **Step 6: Commit the shared-analysis layer**

```bash
git add Cargo.toml crates/pa-analysis
git commit -m "feat: add shared bar analysis and daily context"
```

- [ ] **Step 7: Expand the workspace members for the user crate**

```toml
[workspace]
resolver = "3"
members = [
    "crates/pa-core",
    "crates/pa-instrument",
    "crates/pa-market",
    "crates/pa-analysis",
    "crates/pa-user",
]
```

### Task 7: Add Minimal User Subscriptions, Positions, and User Analysis

**Files:**
- Create: `E:\rust-app\oh-paa\crates\pa-user\Cargo.toml`
- Create: `E:\rust-app\oh-paa\crates\pa-user\src\lib.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-user\src\models.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-user\src\repository.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-user\src\service.rs`
- Test: `E:\rust-app\oh-paa\crates\pa-user\tests\manual_user_analysis.rs`

- [ ] **Step 1: Write the failing user-analysis reuse test**

```rust
use pa_user::service::UserAnalysisService;

#[tokio::test]
async fn manual_user_analysis_uses_shared_outputs_instead_of_provider_calls() {
    let service = UserAnalysisService::new(
        FixtureUserRepo::default(),
        FixtureSharedAnalysisRepo::with_outputs(),
        FixtureLlm::default(),
    );

    let report = service
        .run_manual_analysis(FixtureManualRequest::btc_long())
        .await
        .unwrap();

    assert!(report.prompt_context.contains("daily_market_context"));
    assert!(report.prompt_context.contains("bar_analysis"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p pa-user manual_user_analysis_uses_shared_outputs_instead_of_provider_calls -- --exact`

Expected: FAIL because `pa-user` does not exist yet

- [ ] **Step 3: Implement the user models and repository contract**

```rust
// crates/pa-user/src/models.rs
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UserSubscription {
    pub user_id: Uuid,
    pub instrument_id: Uuid,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct PositionSnapshot {
    pub user_id: Uuid,
    pub instrument_id: Uuid,
    pub side: String,
    pub quantity: Decimal,
    pub average_cost: Decimal,
}
```

- [ ] **Step 4: Implement manual user analysis by composing shared analysis plus positions**

```rust
// crates/pa-user/src/service.rs
pub async fn run_manual_analysis(
    &self,
    request: ManualUserAnalysisRequest,
) -> Result<UserAnalysisReport, AppError> {
    let positions = self.repo.load_positions(request.user_id, request.instrument_id).await?;
    let bar_analysis = self.shared_repo.latest_bar_analysis(request.instrument_id, &request.timeframe).await?;
    let daily_context = self.shared_repo.latest_daily_context(request.instrument_id).await?;

    let prompt_context = serde_json::json!({
        "positions": positions,
        "bar_analysis": bar_analysis,
        "daily_market_context": daily_context,
    });

    self.llm.generate_user_analysis(prompt_context).await
}
```

- [ ] **Step 5: Run the user-analysis test**

Run: `cargo test -p pa-user manual_user_analysis_uses_shared_outputs_instead_of_provider_calls -- --exact`

Expected: PASS

- [ ] **Step 6: Commit the minimal user layer**

```bash
git add Cargo.toml crates/pa-user
git commit -m "feat: add subscriptions positions and manual user analysis"
```

- [ ] **Step 7: Expand the workspace members for the API and app crates**

```toml
[workspace]
resolver = "3"
members = [
    "crates/pa-core",
    "crates/pa-instrument",
    "crates/pa-market",
    "crates/pa-analysis",
    "crates/pa-user",
    "crates/pa-api",
    "crates/pa-app",
]
```

### Task 8: Wire the HTTP API, App Runtime, and Operator Notes

**Files:**
- Create: `E:\rust-app\oh-paa\crates\pa-api\Cargo.toml`
- Create: `E:\rust-app\oh-paa\crates\pa-api\src\lib.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-api\src\router.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-api\src\admin.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-api\src\market.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-api\src\analysis.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-api\src\user.rs`
- Create: `E:\rust-app\oh-paa\crates\pa-app\Cargo.toml`
- Create: `E:\rust-app\oh-paa\crates\pa-app\src\main.rs`
- Create: `E:\rust-app\oh-paa\docs\architecture\phase1-runtime.md`
- Test: `E:\rust-app\oh-paa\crates\pa-api\tests\smoke.rs`

- [ ] **Step 1: Write the failing API smoke test**

```rust
use axum::body::Body;
use axum::http::StatusCode;
use axum::http::Request;
use tower::ServiceExt;

#[tokio::test]
async fn healthcheck_and_market_data_routes_are_wired() {
    let app = pa_api::router::app_router(FixtureState::new());

    let response = app.oneshot(Request::get("/healthz").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Run the smoke test to verify it fails**

Run: `cargo test -p pa-api healthcheck_and_market_data_routes_are_wired -- --exact`

Expected: FAIL because `pa-api` does not exist yet

- [ ] **Step 3: Implement route groups that mirror the approved product boundaries**

```rust
// crates/pa-api/src/router.rs
use axum::{routing::{get, post}, Router};

#[derive(Clone)]
pub struct AppState {
    pub health_label: &'static str,
}

pub fn app_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest("/admin", admin::routes())
        .nest("/market", market::routes())
        .nest("/analysis", analysis::routes())
        .nest("/user", user::routes())
        .with_state(state)
}
```

- [ ] **Step 4: Implement the app crate for config loading, SQLx migration, provider registration, and tracing**

```rust
// crates/pa-app/src/main.rs
use pa_api::router::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let config = pa_core::config::load()?;
    let pool = sqlx::PgPool::connect(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let app_state = build_app_state(pool, config).await?;
    let app = pa_api::router::app_router(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn build_app_state(
    _pool: sqlx::PgPool,
    _config: pa_core::config::AppConfig,
) -> anyhow::Result<AppState> {
    Ok(AppState { health_label: "ok" })
}
```

- [ ] **Step 5: Run smoke tests and a compile check for the full workspace**

Run: `cargo test -p pa-api --test smoke`

Expected: PASS

Run: `cargo check --workspace`

Expected: PASS with all crates compiling

- [ ] **Step 6: Add operator notes and commit the runnable Phase 1 foundation**

```markdown
# Phase 1 Runtime Notes

- database migrations must run before serving traffic
- configure `TwelveData` API key before enabling forex or crypto instruments
- `EastMoney` should be used primarily for A-share coverage
- open bars are derived runtime state and are not historical truth
- use admin backfill endpoints before enabling shared analysis for a new instrument
```

```bash
git add Cargo.toml crates/pa-api crates/pa-app docs/architecture
git commit -m "feat: wire api runtime and phase1 operator notes"
```

## Verification Checklist

Run these commands before claiming the plan has been executed successfully:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected:

- formatting finishes without changing semantic code
- clippy passes with no warnings
- all unit, integration, and smoke tests pass

## Spec Coverage Self-Review

- `instrument_id` as internal truth: covered by Task 2 models and schema.
- market-level default policy plus instrument override: covered by Task 2 policy service and schema.
- `EastMoney + TwelveData`: covered by Task 4 provider adapters.
- primary/fallback provider routing: covered by Task 4 router and tests.
- canonical closed-bar truth: covered by Task 5 repository and idempotent backfill task.
- open bar as derived runtime-only state: covered by Task 5 open-bar book and runtime notes.
- per-bar shared analysis: covered by Task 6 bar-analysis service.
- daily shared market context: covered by Task 6 daily-context worker.
- minimal user subscriptions, positions, and manual analysis: covered by Task 7.
- admin, market, analysis, and user APIs: covered by Task 8.
- observability and runtime wiring: covered by Task 8 main app setup plus verification checklist.

Gaps intentionally deferred to later phases:

- user-defined scheduled triggers
- multi-timezone exact market sessions
- historical tick warehousing
- provider arbitration and quality scoring

## Placeholder Scan

Checked for and removed:

- common placeholder markers
- vague instructions that do not describe a concrete action
- missing command expectations

## Type Consistency Review

- `ProviderPolicy` naming is consistent across Tasks 2, 4, and 5.
- `canonical_kline` is the storage truth across Tasks 2 and 5.
- shared analysis output names `BarAnalysis` and `DailyMarketContext` are reused consistently in Tasks 6 and 7.
- route groups `admin`, `market`, `analysis`, and `user` align with the approved design and Task 8.
