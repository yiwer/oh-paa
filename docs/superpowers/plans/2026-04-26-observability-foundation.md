# Observability Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `pa-observability` crate that provides metrics (Prometheus), distributed traces (OTLP), structured JSON logs, and HTTP health endpoints to oh-paa, with a domain-semantic instrumentation API replacing free-form metric strings.

**Architecture:** New `pa-observability` workspace crate owns initialization (tracing subscriber + Prometheus recorder + optional OTLP layer) and exposes typed recording functions per business domain (orchestration, LLM, market, API, infra). Business crates depend on `pa-observability`; metric name strings stay private. `/metrics`, `/healthz`, `/readyz` are mounted on the existing `pa-api` Axum router.

**Tech Stack:** `metrics` 0.24 + `metrics-exporter-prometheus` 0.16 + `tracing-opentelemetry` 0.30 + `opentelemetry-otlp` 0.30 + `opentelemetry_sdk` 0.30 + `tracing-subscriber` 0.3 (already in workspace).

**Spec:** `docs/superpowers/specs/2026-04-26-observability-foundation-design.md`

---

## File Structure

**New files (`crates/pa-observability/`):**

| Path | Responsibility |
|---|---|
| `Cargo.toml` | crate manifest |
| `src/lib.rs` | public re-exports + crate-level docs |
| `src/config.rs` | `ObservabilityConfig` parsed from env |
| `src/init.rs` | `init()` + `Guard` (subscriber + recorder + optional OTLP) |
| `src/health.rs` | `HealthCheck` trait + `CompositeHealth` |
| `src/http.rs` | `router()` exposing `/metrics`, `/healthz`, `/readyz` |
| `src/domain/mod.rs` | submodule re-exports |
| `src/domain/orchestration.rs` | orchestration metric constants + typed API |
| `src/domain/llm.rs` | LLM metric constants + typed API |
| `src/domain/market.rs` | market metric constants + typed API |
| `src/domain/api.rs` | HTTP request middleware metrics |
| `src/domain/infra.rs` | pg pool gauges |
| `tests/catalog_consistency.rs` | enforces `signals.md` ↔ code parity |

**New docs:**

| Path | Responsibility |
|---|---|
| `docs/observability/signals.md` | canonical signal catalog |
| `docs/observability/runbook.md` | operator validation + diagnostic flows |

**Modified files:**

| Path | Change |
|---|---|
| `Cargo.toml` (workspace) | add `pa-observability` member + new shared deps |
| `crates/pa-app/Cargo.toml` | add `pa-observability` dep |
| `crates/pa-app/src/main.rs` | replace tracing init with `pa_observability::init()` |
| `crates/pa-app/src/lib.rs` | replace tracing init with `pa_observability::init()` |
| `crates/pa-api/Cargo.toml` | add `pa-observability` dep |
| `crates/pa-api/src/router.rs` | merge `pa_observability::router()`, remove old `/healthz` |
| `crates/pa-orchestrator/Cargo.toml` | add `pa-observability` dep |
| `crates/pa-orchestrator/src/...` | call orchestration domain API at task lifecycle points |
| `crates/pa-market/Cargo.toml` | add `pa-observability` dep |
| `crates/pa-market/src/...` | call market domain API at provider fetch points |

---

## Task 1: Create `pa-observability` crate skeleton

**Files:**
- Create: `crates/pa-observability/Cargo.toml`
- Create: `crates/pa-observability/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add new shared dependencies to workspace `Cargo.toml`**

Append under `[workspace.dependencies]`:

```toml
metrics = "0.24"
metrics-exporter-prometheus = { version = "0.16", default-features = false }
opentelemetry = "0.30"
opentelemetry_sdk = { version = "0.30", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.30", features = ["grpc-tonic"] }
tracing-opentelemetry = "0.30"
```

Add `"crates/pa-observability"` to `members`.

- [ ] **Step 2: Create `crates/pa-observability/Cargo.toml`**

```toml
[package]
name = "pa-observability"
version.workspace = true
edition.workspace = true

[dependencies]
async-trait.workspace = true
axum.workspace = true
metrics.workspace = true
metrics-exporter-prometheus.workspace = true
opentelemetry.workspace = true
opentelemetry_sdk.workspace = true
opentelemetry-otlp.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-opentelemetry.workspace = true
tracing-subscriber = { workspace = true, features = ["fmt", "env-filter", "json"] }
```

- [ ] **Step 3: Create `crates/pa-observability/src/lib.rs`**

```rust
//! Observability foundation for oh-paa.
//!
//! Exposes a domain-semantic instrumentation API. Business crates call typed
//! functions in [`domain`] rather than writing metric name strings.

pub mod config;
pub mod domain;
pub mod health;
pub mod http;
pub mod init;

pub use config::ObservabilityConfig;
pub use http::router;
pub use init::{Guard, init};
```

- [ ] **Step 4: Create stubs so the crate compiles**

`crates/pa-observability/src/config.rs`:

```rust
#[derive(Debug, Clone)]
pub struct ObservabilityConfig;

impl ObservabilityConfig {
    pub fn from_env() -> Self {
        Self
    }
}
```

`crates/pa-observability/src/init.rs`:

```rust
use crate::ObservabilityConfig;

#[derive(Debug)]
pub struct Guard;

pub fn init(_config: ObservabilityConfig) -> anyhow::Result<Guard> {
    Ok(Guard)
}
```

Wait — `anyhow` is a workspace dep but not in our crate manifest. Add it:

`crates/pa-observability/Cargo.toml` add `anyhow.workspace = true`.

`crates/pa-observability/src/health.rs`:

```rust
// HealthCheck trait — implemented in Task 4.
```

`crates/pa-observability/src/http.rs`:

```rust
use axum::Router;

pub fn router() -> Router {
    Router::new()
}
```

`crates/pa-observability/src/domain/mod.rs`:

```rust
// Domain modules — implemented in Tasks 7-11.
```

- [ ] **Step 5: Verify the workspace compiles**

Run: `cargo check -p pa-observability`
Expected: clean compile.

Run: `cargo build --workspace`
Expected: clean compile.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/pa-observability/
git commit -m "feat(pa-observability): add crate skeleton"
```

---

## Task 2: Implement `ObservabilityConfig`

**Files:**
- Modify: `crates/pa-observability/src/config.rs`
- Test: `crates/pa-observability/src/config.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Write the failing test**

Replace contents of `crates/pa-observability/src/config.rs`:

```rust
use std::env;

#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub service_name: String,
    pub log_format: LogFormat,
    pub rust_log: String,
    pub otlp_endpoint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Text,
}

impl ObservabilityConfig {
    pub fn from_env() -> Self {
        let log_format = match env::var("LOG_FORMAT").ok().as_deref() {
            Some("text") => LogFormat::Text,
            _ => LogFormat::Json,
        };
        Self {
            service_name: env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "oh-paa".to_string()),
            log_format,
            rust_log: env::var("RUST_LOG").unwrap_or_else(|_| "info,oh_paa=debug".to_string()),
            otlp_endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok().filter(|s| !s.is_empty()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        // SAFETY: tests in this module are gated by a Mutex below.
        let prev: Vec<_> = vars.iter().map(|(k, _)| (*k, env::var(k).ok())).collect();
        for (k, v) in vars {
            match v {
                Some(val) => unsafe { env::set_var(k, val) },
                None => unsafe { env::remove_var(k) },
            }
        }
        f();
        for (k, v) in prev {
            match v {
                Some(val) => unsafe { env::set_var(k, val) },
                None => unsafe { env::remove_var(k) },
            }
        }
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn defaults_when_env_absent() {
        let _g = ENV_LOCK.lock().unwrap();
        with_env(
            &[
                ("OTEL_SERVICE_NAME", None),
                ("LOG_FORMAT", None),
                ("RUST_LOG", None),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
            ],
            || {
                let c = ObservabilityConfig::from_env();
                assert_eq!(c.service_name, "oh-paa");
                assert_eq!(c.log_format, LogFormat::Json);
                assert_eq!(c.rust_log, "info,oh_paa=debug");
                assert_eq!(c.otlp_endpoint, None);
            },
        );
    }

    #[test]
    fn empty_otlp_endpoint_treated_as_unset() {
        let _g = ENV_LOCK.lock().unwrap();
        with_env(&[("OTEL_EXPORTER_OTLP_ENDPOINT", Some(""))], || {
            assert_eq!(ObservabilityConfig::from_env().otlp_endpoint, None);
        });
    }

    #[test]
    fn log_format_text_parsed() {
        let _g = ENV_LOCK.lock().unwrap();
        with_env(&[("LOG_FORMAT", Some("text"))], || {
            assert_eq!(ObservabilityConfig::from_env().log_format, LogFormat::Text);
        });
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p pa-observability config::tests`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/pa-observability/src/config.rs
git commit -m "feat(pa-observability): env-driven ObservabilityConfig"
```

---

## Task 3: Implement `init()` (subscriber + Prometheus recorder, OTLP deferred)

**Files:**
- Modify: `crates/pa-observability/src/init.rs`

- [ ] **Step 1: Implement `init()` without OTLP**

Replace `crates/pa-observability/src/init.rs`:

```rust
use anyhow::Context;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::{LogFormat, ObservabilityConfig};

/// Drop guard that flushes async exporters on shutdown.
#[derive(Debug)]
pub struct Guard {
    pub(crate) prometheus: PrometheusHandle,
    pub(crate) otlp_installed: bool,
}

impl Guard {
    pub fn prometheus_handle(&self) -> &PrometheusHandle {
        &self.prometheus
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if self.otlp_installed {
            opentelemetry::global::shutdown_tracer_provider();
        }
    }
}

pub fn init(config: ObservabilityConfig) -> anyhow::Result<Guard> {
    let env_filter = EnvFilter::try_new(&config.rust_log)
        .with_context(|| format!("invalid RUST_LOG filter: {}", config.rust_log))?;

    let registry = tracing_subscriber::registry().with(env_filter);

    match config.log_format {
        LogFormat::Json => registry
            .with(tracing_subscriber::fmt::layer().json())
            .try_init()
            .map_err(|e| anyhow::anyhow!("tracing init failed: {e}"))?,
        LogFormat::Text => registry
            .with(tracing_subscriber::fmt::layer())
            .try_init()
            .map_err(|e| anyhow::anyhow!("tracing init failed: {e}"))?,
    }

    let prometheus = PrometheusBuilder::new()
        .install_recorder()
        .context("install prometheus recorder")?;

    tracing::info!(
        target: "pa_observability",
        metrics = "on",
        otlp = %if config.otlp_endpoint.is_some() { "pending" } else { "off" },
        service = %config.service_name,
        "observability initialized"
    );

    Ok(Guard {
        prometheus,
        otlp_installed: false,
    })
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p pa-observability`
Expected: clean compile.

- [ ] **Step 3: Add a smoke test**

Append to `crates/pa-observability/src/init.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Subscriber init is global; we only check the call doesn't panic on a
    // process that hasn't initialized one. Run as its own #[test] file would
    // be better, but cargo runs each test binary in a fresh process.

    #[test]
    fn init_returns_guard_holding_handle() {
        let cfg = ObservabilityConfig {
            service_name: "test".to_string(),
            log_format: LogFormat::Text,
            rust_log: "off".to_string(),
            otlp_endpoint: None,
        };
        let guard = init(cfg).expect("init should succeed");
        let rendered = guard.prometheus_handle().render();
        // Empty metrics rendering still returns a valid string (possibly empty).
        assert!(rendered.is_empty() || rendered.contains("# "));
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p pa-observability init::tests`
Expected: 1 test passes.

- [ ] **Step 5: Commit**

```bash
git add crates/pa-observability/src/init.rs
git commit -m "feat(pa-observability): init subscriber + prometheus recorder"
```

---

## Task 4: Implement `HealthCheck` trait + `CompositeHealth`

**Files:**
- Modify: `crates/pa-observability/src/health.rs`

- [ ] **Step 1: Write the failing test**

Replace `crates/pa-observability/src/health.rs`:

```rust
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy(String),
}

#[async_trait]
pub trait HealthCheck: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    async fn check(&self) -> HealthStatus;
}

#[derive(Default, Clone)]
pub struct CompositeHealth {
    checks: Vec<Arc<dyn HealthCheck>>,
}

impl CompositeHealth {
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    pub fn register<C: HealthCheck>(&mut self, check: C) -> &mut Self {
        self.checks.push(Arc::new(check));
        self
    }

    /// Returns Ok(()) if all healthy, Err(list of failed check names + reasons) otherwise.
    pub async fn evaluate(&self) -> Result<(), Vec<(String, String)>> {
        let mut failures = Vec::new();
        for check in &self.checks {
            if let HealthStatus::Unhealthy(reason) = check.check().await {
                failures.push((check.name().to_string(), reason));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Always(HealthStatus, &'static str);
    #[async_trait]
    impl HealthCheck for Always {
        fn name(&self) -> &'static str {
            self.1
        }
        async fn check(&self) -> HealthStatus {
            self.0.clone()
        }
    }

    #[tokio::test]
    async fn empty_composite_is_healthy() {
        assert_eq!(CompositeHealth::new().evaluate().await, Ok(()));
    }

    #[tokio::test]
    async fn single_failure_reported() {
        let mut c = CompositeHealth::new();
        c.register(Always(HealthStatus::Healthy, "ok-one"));
        c.register(Always(HealthStatus::Unhealthy("down".into()), "bad"));
        let err = c.evaluate().await.unwrap_err();
        assert_eq!(err, vec![("bad".to_string(), "down".to_string())]);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p pa-observability health::tests`
Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/pa-observability/src/health.rs
git commit -m "feat(pa-observability): HealthCheck trait + CompositeHealth"
```

---

## Task 5: Implement HTTP router (`/metrics`, `/healthz`, `/readyz`)

**Files:**
- Modify: `crates/pa-observability/src/http.rs`
- Modify: `crates/pa-observability/src/lib.rs` (re-exports)

- [ ] **Step 1: Implement router**

Replace `crates/pa-observability/src/http.rs`:

```rust
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use metrics_exporter_prometheus::PrometheusHandle;
use serde_json::json;

use crate::health::CompositeHealth;

#[derive(Clone)]
pub struct ObservabilityState {
    pub prometheus: PrometheusHandle,
    pub health: Arc<CompositeHealth>,
}

pub fn router(state: ObservabilityState) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .with_state(state)
}

async fn metrics_handler(State(s): State<ObservabilityState>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        s.prometheus.render(),
    )
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn readyz(State(s): State<ObservabilityState>) -> impl IntoResponse {
    match s.health.evaluate().await {
        Ok(()) => (StatusCode::OK, Json(json!({ "status": "ready" }))).into_response(),
        Err(failures) => {
            let body = json!({
                "status": "not_ready",
                "failed": failures
                    .into_iter()
                    .map(|(name, reason)| json!({ "name": name, "reason": reason }))
                    .collect::<Vec<_>>(),
            });
            (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response()
        }
    }
}
```

- [ ] **Step 2: Update `init` to expose state for the router**

In `crates/pa-observability/src/init.rs`, change `Guard` and add a `into_state` helper. Replace the `Guard` definition and impl with:

```rust
use std::sync::Arc;

use crate::health::CompositeHealth;
use crate::http::ObservabilityState;

#[derive(Debug)]
pub struct Guard {
    pub(crate) prometheus: PrometheusHandle,
    pub(crate) otlp_installed: bool,
}

impl Guard {
    pub fn prometheus_handle(&self) -> &PrometheusHandle {
        &self.prometheus
    }

    pub fn observability_state(&self, health: Arc<CompositeHealth>) -> ObservabilityState {
        ObservabilityState {
            prometheus: self.prometheus.clone(),
            health,
        }
    }
}
```

- [ ] **Step 3: Update `lib.rs` re-exports**

Replace `crates/pa-observability/src/lib.rs`:

```rust
//! Observability foundation for oh-paa.

pub mod config;
pub mod domain;
pub mod health;
pub mod http;
pub mod init;

pub use config::{LogFormat, ObservabilityConfig};
pub use health::{CompositeHealth, HealthCheck, HealthStatus};
pub use http::{ObservabilityState, router};
pub use init::{Guard, init};
```

- [ ] **Step 4: Add HTTP integration tests**

Add `crates/pa-observability/Cargo.toml` dev-deps:

```toml
[dev-dependencies]
http-body-util = "0.1"
tower = "0.5"
```

Create `crates/pa-observability/tests/http.rs`:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use metrics_exporter_prometheus::PrometheusBuilder;
use pa_observability::{
    CompositeHealth, HealthCheck, HealthStatus, ObservabilityState, router,
};
use tower::ServiceExt;

struct Failing;
#[async_trait]
impl HealthCheck for Failing {
    fn name(&self) -> &'static str {
        "always_fails"
    }
    async fn check(&self) -> HealthStatus {
        HealthStatus::Unhealthy("synthetic".into())
    }
}

fn build(health: CompositeHealth) -> axum::Router {
    let prometheus = PrometheusBuilder::new()
        .build_recorder()
        .handle();
    router(ObservabilityState {
        prometheus,
        health: Arc::new(health),
    })
}

#[tokio::test]
async fn healthz_always_ok() {
    let app = build(CompositeHealth::new());
    let res = app
        .oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn readyz_ok_when_no_failures() {
    let app = build(CompositeHealth::new());
    let res = app
        .oneshot(Request::builder().uri("/readyz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn readyz_503_when_check_fails() {
    let mut h = CompositeHealth::new();
    h.register(Failing);
    let app = build(h);
    let res = app
        .oneshot(Request::builder().uri("/readyz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["status"], "not_ready");
    assert_eq!(v["failed"][0]["name"], "always_fails");
    assert_eq!(v["failed"][0]["reason"], "synthetic");
}

#[tokio::test]
async fn metrics_endpoint_returns_text() {
    let app = build(CompositeHealth::new());
    let res = app
        .oneshot(Request::builder().uri("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let ct = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.starts_with("text/plain"));
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p pa-observability --test http`
Expected: 4 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pa-observability/
git commit -m "feat(pa-observability): HTTP router for metrics + health endpoints"
```

---

## Task 6: Wire `pa-app` and `pa-api` to the observability foundation

**Files:**
- Modify: `crates/pa-app/Cargo.toml`
- Modify: `crates/pa-app/src/main.rs`
- Modify: `crates/pa-app/src/lib.rs`
- Modify: `crates/pa-api/Cargo.toml`
- Modify: `crates/pa-api/src/router.rs`

- [ ] **Step 1: Add `pa-observability` dep to `pa-app` and `pa-api`**

`crates/pa-app/Cargo.toml` `[dependencies]` add:

```toml
pa-observability = { path = "../pa-observability" }
```

`crates/pa-api/Cargo.toml` `[dependencies]` add:

```toml
pa-observability = { path = "../pa-observability" }
```

- [ ] **Step 2: Replace tracing init in `pa-app/src/main.rs`**

Find the existing block (around lines 16-25) that calls `tracing_subscriber::fmt().init()` and replace it with:

```rust
use pa_observability::{CompositeHealth, ObservabilityConfig};
use std::sync::Arc;

let _observability_guard = pa_observability::init(ObservabilityConfig::from_env())
    .expect("init observability");
let health = Arc::new(CompositeHealth::new());
let observability_state = _observability_guard.observability_state(health.clone());
```

Remove the `use tracing_subscriber::EnvFilter;` import.

The `_observability_guard` must outlive the entire `main` body so OTLP flushing works on shutdown — keep it bound until the end.

- [ ] **Step 3: Replace tracing init in `pa-app/src/lib.rs` (test helper)**

The existing `try_init` helper in `lib.rs` is for tests. Replace its body to delegate:

```rust
pub fn init_tracing_for_tests() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = pa_observability::init(pa_observability::ObservabilityConfig {
            service_name: "oh-paa-tests".to_string(),
            log_format: pa_observability::LogFormat::Text,
            rust_log: std::env::var("RUST_LOG").unwrap_or_else(|_| "off".to_string()),
            otlp_endpoint: None,
        });
    });
}
```

Update the existing function name if it differs; keep the call sites unchanged by re-exporting under the previous name.

- [ ] **Step 4: Update `pa-api/src/router.rs` to merge observability router**

Replace the `app_router` function (lines 75-83) and the `healthz` helper (lines 85-87) with:

```rust
pub fn app_router(state: AppState, observability: pa_observability::ObservabilityState) -> Router {
    Router::new()
        .merge(pa_observability::router(observability))
        .nest("/admin", admin::routes())
        .nest("/market", market::routes())
        .nest("/analysis", analysis::routes())
        .nest("/user", user::routes())
        .with_state(state)
}
```

The old `/healthz` is removed; the merged router brings the new one.

- [ ] **Step 5: Update `pa-app` call site that constructs the router**

In `pa-app/src/main.rs` find the `app_router(state)` call and pass the observability state:

```rust
let app = pa_api::app_router(app_state, observability_state);
```

- [ ] **Step 6: Verify everything compiles and tests pass**

Run: `cargo build --workspace`
Expected: clean build.

Run: `cargo test --workspace`
Expected: all existing tests still pass; if any test in `pa-api` constructed `app_router(state)` directly, update it to pass a default `ObservabilityState`. Add a test helper in `pa-api` if needed:

```rust
// In pa-api dev-deps or test module:
fn test_observability_state() -> pa_observability::ObservabilityState {
    use std::sync::Arc;
    let prometheus = metrics_exporter_prometheus::PrometheusBuilder::new()
        .build_recorder()
        .handle();
    pa_observability::ObservabilityState {
        prometheus,
        health: Arc::new(pa_observability::CompositeHealth::new()),
    }
}
```

Wire that helper into existing `pa-api` integration tests at the `app_router(state)` call sites.

- [ ] **Step 7: Manual smoke test**

Run: `cargo run -p pa-app` (with whatever args your local config requires).
In another shell:

```bash
curl -i http://127.0.0.1:8080/healthz   # adjust port if different
curl -i http://127.0.0.1:8080/readyz
curl -s http://127.0.0.1:8080/metrics | head -20
```

Expected: `/healthz` 200 "ok"; `/readyz` 200 with `{"status":"ready"}`; `/metrics` returns prometheus text format (may be empty until task 7+ adds signals).

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/pa-app/ crates/pa-api/
git commit -m "feat: wire pa-observability into pa-app and pa-api"
```

---

## Task 7: Domain — Orchestration

**Files:**
- Modify: `crates/pa-observability/src/domain/mod.rs`
- Create: `crates/pa-observability/src/domain/orchestration.rs`
- Modify: `crates/pa-orchestrator/Cargo.toml`
- Modify: `crates/pa-orchestrator/src/...` (instrumentation call sites)
- Modify: `docs/observability/signals.md`

- [ ] **Step 1: Implement orchestration domain API**

Create `crates/pa-observability/src/domain/orchestration.rs`:

```rust
use std::time::Duration;

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};

const TASKS_TOTAL: &str = "orchestration_tasks_total";
const QUEUE_DEPTH: &str = "orchestration_queue_depth";
const CLAIM_DURATION: &str = "orchestration_claim_duration_seconds";
const TASK_DURATION: &str = "orchestration_task_duration_seconds";
const ATTEMPTS_PER_TASK: &str = "orchestration_attempts_per_task";
const DEAD_LETTER_TOTAL: &str = "orchestration_dead_letter_total";

pub fn describe_all() {
    describe_counter!(TASKS_TOTAL, "Orchestration task state transitions.");
    describe_gauge!(QUEUE_DEPTH, "Current orchestration queue depth by state.");
    describe_histogram!(
        CLAIM_DURATION,
        metrics::Unit::Seconds,
        "Time spent in claim_next_pending_task."
    );
    describe_histogram!(
        TASK_DURATION,
        metrics::Unit::Seconds,
        "End-to-end task duration."
    );
    describe_histogram!(ATTEMPTS_PER_TASK, "Retry attempts per task.");
    describe_counter!(DEAD_LETTER_TOTAL, "Tasks moved to the dead-letter queue.");
}

#[derive(Debug, Clone, Copy)]
pub enum TaskStatus {
    Pending,
    Claimed,
    Completed,
    Failed,
    DeadLettered,
}

impl TaskStatus {
    fn as_label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Claimed => "claimed",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::DeadLettered => "dead_lettered",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum QueueState {
    Pending,
    Claimed,
    Dead,
}

impl QueueState {
    fn as_label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Claimed => "claimed",
            Self::Dead => "dead",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TaskOutcome {
    Success,
    Failure,
    DeadLetter,
}

impl TaskOutcome {
    fn as_label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::DeadLetter => "dead_letter",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DeadLetterReason {
    SchemaValidation,
    MaxAttemptsExceeded,
    NonRetryableError,
}

impl DeadLetterReason {
    fn as_label(self) -> &'static str {
        match self {
            Self::SchemaValidation => "schema_validation",
            Self::MaxAttemptsExceeded => "max_attempts_exceeded",
            Self::NonRetryableError => "non_retryable_error",
        }
    }
}

pub fn task_state_transition(status: TaskStatus, prompt_key: &str) {
    counter!(
        TASKS_TOTAL,
        "status" => status.as_label(),
        "prompt_key" => prompt_key.to_string(),
    )
    .increment(1);
}

pub fn queue_depth(state: QueueState, depth: u64) {
    gauge!(QUEUE_DEPTH, "state" => state.as_label()).set(depth as f64);
}

pub fn claim_duration(d: Duration) {
    histogram!(CLAIM_DURATION).record(d.as_secs_f64());
}

pub fn task_duration(prompt_key: &str, outcome: TaskOutcome, d: Duration) {
    histogram!(
        TASK_DURATION,
        "prompt_key" => prompt_key.to_string(),
        "outcome" => outcome.as_label(),
    )
    .record(d.as_secs_f64());
}

pub fn attempts_per_task(prompt_key: &str, attempts: u32) {
    histogram!(ATTEMPTS_PER_TASK, "prompt_key" => prompt_key.to_string())
        .record(attempts as f64);
}

pub fn dead_letter(reason: DeadLetterReason) {
    counter!(DEAD_LETTER_TOTAL, "reason" => reason.as_label()).increment(1);
}

pub const METRIC_NAMES: &[&str] = &[
    TASKS_TOTAL,
    QUEUE_DEPTH,
    CLAIM_DURATION,
    TASK_DURATION,
    ATTEMPTS_PER_TASK,
    DEAD_LETTER_TOTAL,
];

#[cfg(test)]
mod tests {
    use super::*;
    use metrics_util::debugging::{DebuggingRecorder, Snapshotter};

    fn snap_with<F: FnOnce()>(f: F) -> Snapshotter {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || f());
        snapshotter
    }

    #[test]
    fn task_state_transition_records_counter() {
        let s = snap_with(|| {
            task_state_transition(TaskStatus::Completed, "shared_pa_state_bar_v1");
        });
        let snap = s.snapshot();
        let entries: Vec<_> = snap.into_vec();
        let found = entries.iter().any(|(key, _, _, _)| {
            let k = key.key();
            k.name() == TASKS_TOTAL
                && k.labels().any(|l| l.key() == "status" && l.value() == "completed")
                && k.labels().any(|l| l.key() == "prompt_key" && l.value() == "shared_pa_state_bar_v1")
        });
        assert!(found, "expected counter entry not in snapshot: {entries:?}");
    }

    #[test]
    fn task_duration_records_histogram() {
        let s = snap_with(|| {
            task_duration(
                "p",
                TaskOutcome::Success,
                Duration::from_millis(123),
            );
        });
        let snap = s.snapshot();
        let entries = snap.into_vec();
        assert!(entries.iter().any(|(key, _, _, _)| key.key().name() == TASK_DURATION));
    }
}
```

Add `metrics-util` to `pa-observability` dev-deps:

`crates/pa-observability/Cargo.toml`:

```toml
[dev-dependencies]
http-body-util = "0.1"
metrics-util = { version = "0.19", features = ["debugging"] }
tower = "0.5"
```

- [ ] **Step 2: Update `domain/mod.rs`**

Replace `crates/pa-observability/src/domain/mod.rs`:

```rust
pub mod orchestration;

/// Call once during init to register all metric descriptions with the recorder.
pub fn describe_all() {
    orchestration::describe_all();
}
```

- [ ] **Step 3: Call `domain::describe_all()` from `init`**

In `crates/pa-observability/src/init.rs`, after `install_recorder()` succeeds and before the `tracing::info!`, add:

```rust
crate::domain::describe_all();
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p pa-observability orchestration`
Expected: 2 tests pass.

- [ ] **Step 5: Add `pa-observability` dep to `pa-orchestrator`**

`crates/pa-orchestrator/Cargo.toml` `[dependencies]`:

```toml
pa-observability = { path = "../pa-observability" }
```

- [ ] **Step 6: Insert instrumentation call sites in `pa-orchestrator`**

Search for the orchestration lifecycle points and call the typed API:

```bash
# Use these as discovery anchors (read each file, locate the right spot):
#   - claim_next_pending_task implementation        → claim_duration
#   - task transition to Completed/Failed/DeadLettered → task_state_transition + task_duration
#   - dead-letter insertion                         → dead_letter
#   - retry attempt creation                        → attempts_per_task (record on terminal state)
#   - background loop that polls queue depth (if any) → queue_depth
```

For each point, add:

```rust
use pa_observability::domain::orchestration as obs_orch;

let started = std::time::Instant::now();
// ... existing claim_next_pending_task body ...
obs_orch::claim_duration(started.elapsed());
```

```rust
obs_orch::task_state_transition(obs_orch::TaskStatus::Completed, &task.prompt_key);
obs_orch::task_duration(
    &task.prompt_key,
    obs_orch::TaskOutcome::Success,
    task_started_at.elapsed(),
);
obs_orch::attempts_per_task(&task.prompt_key, attempt_count);
```

```rust
obs_orch::dead_letter(obs_orch::DeadLetterReason::SchemaValidation);
```

The exact files depend on existing structure; search for the symbols
`claim_next_pending_task`, `mark_completed`, `move_to_dead_letter` (or
equivalents) in `crates/pa-orchestrator/src/`.

- [ ] **Step 7: Add an integration test verifying instrumentation**

Find an existing `pa-orchestrator` integration test that runs a task
through to completion. Wrap its body with a local debugging recorder and
assert metrics moved. New file `crates/pa-orchestrator/tests/observability_smoke.rs`:

```rust
use metrics_util::debugging::DebuggingRecorder;

#[tokio::test]
async fn task_completion_records_orchestration_metrics() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    // Locate the closest existing integration test in
    // `crates/pa-orchestrator/tests/` that drives one task end-to-end
    // (claim → execute → save_result) using
    // `InMemoryOrchestrationRepository`. Extract its setup into a reusable
    // fixture function (e.g., `pub async fn run_one_task_to_completion()`).
    // Then call it inside the recorder closure:
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    metrics::with_local_recorder(&recorder, || {
        rt.block_on(async {
            crate::fixtures::run_one_task_to_completion().await;
        });
    });
    let snap = snapshotter.snapshot().into_vec();
    assert!(
        snap.iter().any(|(k, _, _, _)| k.key().name() == "orchestration_task_duration_seconds"),
        "expected orchestration_task_duration_seconds, got: {:?}",
        snap.iter().map(|(k, _, _, _)| k.key().name().to_string()).collect::<Vec<_>>()
    );
    assert!(
        snap.iter().any(|(k, _, _, _)| k.key().name() == "orchestration_tasks_total"),
        "expected orchestration_tasks_total"
    );
}
```

Add `metrics`, `metrics-util` dev-deps to `pa-orchestrator/Cargo.toml`:

```toml
[dev-dependencies]
metrics = "0.24"
metrics-util = { version = "0.19", features = ["debugging"] }
```

- [ ] **Step 8: Run tests**

Run: `cargo test -p pa-orchestrator`
Expected: all existing tests pass + new smoke test passes.

- [ ] **Step 9: Update `signals.md`**

Create `docs/observability/signals.md`:

```markdown
# oh-paa Signal Catalog

This file is the canonical list of metrics exposed by the system. The
catalog-consistency test in `pa-observability` enforces that this file and
the code agree on metric names.

## Conventions

- All time durations use seconds (`_seconds` histograms).
- Counters use the `_total` suffix.
- Label values come from typed enums; raw user input never reaches a label.

## Orchestration

| Metric | Type | Labels |
|---|---|---|
| `orchestration_tasks_total` | counter | `status`, `prompt_key` |
| `orchestration_queue_depth` | gauge | `state` |
| `orchestration_claim_duration_seconds` | histogram | — |
| `orchestration_task_duration_seconds` | histogram | `prompt_key`, `outcome` |
| `orchestration_attempts_per_task` | histogram | `prompt_key` |
| `orchestration_dead_letter_total` | counter | `reason` |

## LLM
*(populated in Task 8)*

## Market
*(populated in Task 9)*

## API
*(populated in Task 10)*

## Infrastructure
*(populated in Task 11)*
```

- [ ] **Step 10: Commit**

```bash
git add crates/pa-observability/ crates/pa-orchestrator/ docs/observability/
git commit -m "feat(observability): orchestration domain metrics"
```

---

## Task 8: Domain — LLM

**Files:**
- Create: `crates/pa-observability/src/domain/llm.rs`
- Modify: `crates/pa-observability/src/domain/mod.rs`
- Modify: `crates/pa-orchestrator/src/...` (LLM client call sites)
- Modify: `docs/observability/signals.md`

- [ ] **Step 1: Implement LLM domain API**

Create `crates/pa-observability/src/domain/llm.rs`:

```rust
use std::time::Duration;

use metrics::{counter, describe_counter, describe_histogram, histogram};

const REQUEST_DURATION: &str = "llm_request_duration_seconds";
const TOKENS_TOTAL: &str = "llm_tokens_total";
const SCHEMA_VALIDATION_TOTAL: &str = "llm_schema_validation_total";
const RETRY_TOTAL: &str = "llm_retry_total";

pub fn describe_all() {
    describe_histogram!(
        REQUEST_DURATION,
        metrics::Unit::Seconds,
        "LLM request latency."
    );
    describe_counter!(TOKENS_TOTAL, "LLM token usage by direction.");
    describe_counter!(
        SCHEMA_VALIDATION_TOTAL,
        "LLM JSON schema validation outcomes."
    );
    describe_counter!(RETRY_TOTAL, "LLM retry attempts by reason.");
}

#[derive(Debug, Clone, Copy)]
pub enum CallOutcome {
    Success,
    Transient,
    RateLimited,
    SchemaInvalid,
    Permanent,
}

impl CallOutcome {
    fn as_label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Transient => "transient",
            Self::RateLimited => "rate_limited",
            Self::SchemaInvalid => "schema_invalid",
            Self::Permanent => "permanent",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TokenKind {
    Input,
    Output,
}

impl TokenKind {
    fn as_label(self) -> &'static str {
        match self {
            Self::Input => "in",
            Self::Output => "out",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SchemaOutcome {
    Pass,
    Fail,
}

impl SchemaOutcome {
    fn as_label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RetryReason {
    Transient,
    RateLimited,
    Schema,
}

impl RetryReason {
    fn as_label(self) -> &'static str {
        match self {
            Self::Transient => "transient",
            Self::RateLimited => "rate_limited",
            Self::Schema => "schema",
        }
    }
}

pub fn request_duration(provider: &str, model: &str, outcome: CallOutcome, d: Duration) {
    histogram!(
        REQUEST_DURATION,
        "provider" => provider.to_string(),
        "model" => model.to_string(),
        "outcome" => outcome.as_label(),
    )
    .record(d.as_secs_f64());
}

pub fn tokens(provider: &str, model: &str, kind: TokenKind, n: u64) {
    counter!(
        TOKENS_TOTAL,
        "provider" => provider.to_string(),
        "model" => model.to_string(),
        "kind" => kind.as_label(),
    )
    .increment(n);
}

pub fn schema_validation(prompt_key: &str, outcome: SchemaOutcome) {
    counter!(
        SCHEMA_VALIDATION_TOTAL,
        "prompt_key" => prompt_key.to_string(),
        "outcome" => outcome.as_label(),
    )
    .increment(1);
}

pub fn retry(provider: &str, reason: RetryReason) {
    counter!(
        RETRY_TOTAL,
        "provider" => provider.to_string(),
        "reason" => reason.as_label(),
    )
    .increment(1);
}

pub const METRIC_NAMES: &[&str] = &[
    REQUEST_DURATION,
    TOKENS_TOTAL,
    SCHEMA_VALIDATION_TOTAL,
    RETRY_TOTAL,
];

#[cfg(test)]
mod tests {
    use super::*;
    use metrics_util::debugging::DebuggingRecorder;

    #[test]
    fn request_duration_recorded() {
        let recorder = DebuggingRecorder::new();
        let snap = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || {
            request_duration("deepseek", "deepseek-reasoner", CallOutcome::Success, Duration::from_millis(50));
        });
        assert!(snap.snapshot().into_vec().iter().any(|(k, _, _, _)| k.key().name() == REQUEST_DURATION));
    }

    #[test]
    fn tokens_recorded() {
        let recorder = DebuggingRecorder::new();
        let snap = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || {
            tokens("openai", "gpt-x", TokenKind::Input, 1234);
        });
        assert!(snap.snapshot().into_vec().iter().any(|(k, _, _, _)| k.key().name() == TOKENS_TOTAL));
    }
}
```

- [ ] **Step 2: Update `domain/mod.rs`**

Replace contents:

```rust
pub mod llm;
pub mod orchestration;

pub fn describe_all() {
    llm::describe_all();
    orchestration::describe_all();
}
```

- [ ] **Step 3: Run unit tests**

Run: `cargo test -p pa-observability llm`
Expected: 2 tests pass.

- [ ] **Step 4: Insert call sites in `pa-orchestrator` LLM client wrapper**

Find the LLM client adapter (typically `pa-orchestrator/src/llm/...` or
similar). Around the actual HTTP call:

```rust
use pa_observability::domain::llm as obs_llm;

let started = std::time::Instant::now();
let result = client.call(&request).await;
let outcome = match &result {
    Ok(_) => obs_llm::CallOutcome::Success,
    Err(e) if e.is_rate_limited() => obs_llm::CallOutcome::RateLimited,
    Err(e) if e.is_transient() => obs_llm::CallOutcome::Transient,
    Err(e) if e.is_schema() => obs_llm::CallOutcome::SchemaInvalid,
    Err(_) => obs_llm::CallOutcome::Permanent,
};
obs_llm::request_duration(provider_name, model_name, outcome, started.elapsed());

if let Ok(resp) = &result {
    if let Some(usage) = resp.token_usage() {
        obs_llm::tokens(provider_name, model_name, obs_llm::TokenKind::Input, usage.input);
        obs_llm::tokens(provider_name, model_name, obs_llm::TokenKind::Output, usage.output);
    }
}
```

Wherever schema validation runs:

```rust
obs_llm::schema_validation(prompt_key, if valid {
    obs_llm::SchemaOutcome::Pass
} else {
    obs_llm::SchemaOutcome::Fail
});
```

Wherever a retry decision is made:

```rust
obs_llm::retry(provider_name, obs_llm::RetryReason::RateLimited);
```

If `token_usage()` and the error classifiers don't exist on the existing
LLM client types, add minimal accessors that return the data already in
the response/error rather than expanding the surface.

- [ ] **Step 5: Update `signals.md`**

Replace the `## LLM` section in `docs/observability/signals.md`:

```markdown
## LLM

| Metric | Type | Labels |
|---|---|---|
| `llm_request_duration_seconds` | histogram | `provider`, `model`, `outcome` |
| `llm_tokens_total` | counter | `provider`, `model`, `kind` |
| `llm_schema_validation_total` | counter | `prompt_key`, `outcome` |
| `llm_retry_total` | counter | `provider`, `reason` |
```

- [ ] **Step 6: Run all tests**

Run: `cargo test --workspace`
Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add crates/pa-observability/ crates/pa-orchestrator/ docs/observability/signals.md
git commit -m "feat(observability): LLM domain metrics"
```

---

## Task 9: Domain — Market

**Files:**
- Create: `crates/pa-observability/src/domain/market.rs`
- Modify: `crates/pa-observability/src/domain/mod.rs`
- Modify: `crates/pa-market/Cargo.toml`
- Modify: `crates/pa-market/src/...` (provider call sites)
- Modify: `docs/observability/signals.md`

- [ ] **Step 1: Implement market domain API**

Create `crates/pa-observability/src/domain/market.rs`:

```rust
use std::time::Duration;

use metrics::{counter, describe_counter, describe_histogram, histogram};

const PROVIDER_REQUESTS_TOTAL: &str = "market_provider_requests_total";
const PROVIDER_DURATION: &str = "market_provider_duration_seconds";
const BARS_INGESTED_TOTAL: &str = "market_bars_ingested_total";

pub fn describe_all() {
    describe_counter!(PROVIDER_REQUESTS_TOTAL, "Market data provider request count.");
    describe_histogram!(
        PROVIDER_DURATION,
        metrics::Unit::Seconds,
        "Market data provider latency."
    );
    describe_counter!(BARS_INGESTED_TOTAL, "Canonical bars persisted.");
}

#[derive(Debug, Clone, Copy)]
pub enum ProviderOutcome {
    Success,
    HttpError,
    ParseError,
    Timeout,
}

impl ProviderOutcome {
    fn as_label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::HttpError => "http_error",
            Self::ParseError => "parse_error",
            Self::Timeout => "timeout",
        }
    }
}

pub fn provider_request(provider: &str, outcome: ProviderOutcome) {
    counter!(
        PROVIDER_REQUESTS_TOTAL,
        "provider" => provider.to_string(),
        "outcome" => outcome.as_label(),
    )
    .increment(1);
}

pub fn provider_duration(provider: &str, d: Duration) {
    histogram!(PROVIDER_DURATION, "provider" => provider.to_string())
        .record(d.as_secs_f64());
}

pub fn bars_ingested(market: &str, timeframe: &str, n: u64) {
    counter!(
        BARS_INGESTED_TOTAL,
        "market" => market.to_string(),
        "timeframe" => timeframe.to_string(),
    )
    .increment(n);
}

pub const METRIC_NAMES: &[&str] = &[
    PROVIDER_REQUESTS_TOTAL,
    PROVIDER_DURATION,
    BARS_INGESTED_TOTAL,
];

#[cfg(test)]
mod tests {
    use super::*;
    use metrics_util::debugging::DebuggingRecorder;

    #[test]
    fn provider_request_recorded() {
        let recorder = DebuggingRecorder::new();
        let snap = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || {
            provider_request("eastmoney", ProviderOutcome::Success);
        });
        assert!(snap.snapshot().into_vec().iter().any(|(k, _, _, _)| k.key().name() == PROVIDER_REQUESTS_TOTAL));
    }

    #[test]
    fn bars_ingested_recorded() {
        let recorder = DebuggingRecorder::new();
        let snap = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || {
            bars_ingested("a-share", "D1", 42);
        });
        assert!(snap.snapshot().into_vec().iter().any(|(k, _, _, _)| k.key().name() == BARS_INGESTED_TOTAL));
    }
}
```

- [ ] **Step 2: Update `domain/mod.rs`**

```rust
pub mod llm;
pub mod market;
pub mod orchestration;

pub fn describe_all() {
    llm::describe_all();
    market::describe_all();
    orchestration::describe_all();
}
```

- [ ] **Step 3: Add `pa-observability` dep to `pa-market`**

`crates/pa-market/Cargo.toml` `[dependencies]`:

```toml
pa-observability = { path = "../pa-observability" }
```

- [ ] **Step 4: Insert call sites in `pa-market` provider adapters**

In each provider adapter (`eastmoney`, `twelvedata`):

```rust
use pa_observability::domain::market as obs_market;

let started = std::time::Instant::now();
let result = self.fetch_inner(&req).await;
let outcome = match &result {
    Ok(_) => obs_market::ProviderOutcome::Success,
    Err(e) if e.is_timeout() => obs_market::ProviderOutcome::Timeout,
    Err(e) if e.is_parse() => obs_market::ProviderOutcome::ParseError,
    Err(_) => obs_market::ProviderOutcome::HttpError,
};
obs_market::provider_request(self.name(), outcome);
obs_market::provider_duration(self.name(), started.elapsed());
```

In the canonical kline persistence path (where rows are inserted):

```rust
obs_market::bars_ingested(market_label, timeframe_label, inserted_count as u64);
```

- [ ] **Step 5: Update `signals.md`**

Replace the `## Market` section:

```markdown
## Market

| Metric | Type | Labels |
|---|---|---|
| `market_provider_requests_total` | counter | `provider`, `outcome` |
| `market_provider_duration_seconds` | histogram | `provider` |
| `market_bars_ingested_total` | counter | `market`, `timeframe` |
```

- [ ] **Step 6: Run tests**

Run: `cargo test --workspace`
Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add crates/pa-observability/ crates/pa-market/ docs/observability/signals.md
git commit -m "feat(observability): market domain metrics"
```

---

## Task 10: Domain — API HTTP middleware

**Files:**
- Create: `crates/pa-observability/src/domain/api.rs`
- Modify: `crates/pa-observability/src/domain/mod.rs`
- Modify: `crates/pa-api/src/router.rs`
- Modify: `docs/observability/signals.md`

- [ ] **Step 1: Implement API domain + axum middleware**

Create `crates/pa-observability/src/domain/api.rs`:

```rust
use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use metrics::{counter, describe_counter, describe_histogram, histogram};

const REQUESTS_TOTAL: &str = "http_requests_total";
const REQUEST_DURATION: &str = "http_request_duration_seconds";

pub fn describe_all() {
    describe_counter!(REQUESTS_TOTAL, "HTTP request count by route, method, and status.");
    describe_histogram!(
        REQUEST_DURATION,
        metrics::Unit::Seconds,
        "HTTP request duration by route."
    );
}

pub async fn track_metrics(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let method = req.method().to_string();

    let response = next.run(req).await;

    let status = response.status().as_u16().to_string();
    let elapsed = start.elapsed().as_secs_f64();

    counter!(
        REQUESTS_TOTAL,
        "route" => route.clone(),
        "method" => method,
        "status" => status,
    )
    .increment(1);
    histogram!(REQUEST_DURATION, "route" => route).record(elapsed);

    response
}

pub fn classify_status(status: StatusCode) -> &'static str {
    match status.as_u16() {
        200..=299 => "2xx",
        300..=399 => "3xx",
        400..=499 => "4xx",
        500..=599 => "5xx",
        _ => "other",
    }
}

pub const METRIC_NAMES: &[&str] = &[REQUESTS_TOTAL, REQUEST_DURATION];

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpReq;
    use axum::routing::get;
    use axum::Router;
    use metrics_util::debugging::DebuggingRecorder;
    use tower::ServiceExt;

    #[tokio::test]
    async fn middleware_records_request() {
        let recorder = DebuggingRecorder::new();
        let snap = recorder.snapshotter();

        let app = Router::new()
            .route("/x", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(track_metrics));

        metrics::with_local_recorder(&recorder, || {
            // axum oneshot is async — drive it via runtime block
        });

        // Outside the recorder closure: drive the request inside it.
        // Easier: run the whole request flow inside `with_local_recorder`.
        metrics::with_local_recorder(&recorder, || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                app.oneshot(HttpReq::builder().uri("/x").body(Body::empty()).unwrap())
                    .await
                    .unwrap();
            });
        });

        let entries = snap.snapshot().into_vec();
        assert!(entries.iter().any(|(k, _, _, _)| k.key().name() == REQUESTS_TOTAL));
        assert!(entries.iter().any(|(k, _, _, _)| k.key().name() == REQUEST_DURATION));
    }
}
```

- [ ] **Step 2: Update `domain/mod.rs`**

```rust
pub mod api;
pub mod llm;
pub mod market;
pub mod orchestration;

pub fn describe_all() {
    api::describe_all();
    llm::describe_all();
    market::describe_all();
    orchestration::describe_all();
}
```

- [ ] **Step 3: Wire middleware into `pa-api/src/router.rs`**

In `app_router`, add the middleware layer:

```rust
pub fn app_router(state: AppState, observability: pa_observability::ObservabilityState) -> Router {
    use axum::middleware;
    use pa_observability::domain::api::track_metrics;
    Router::new()
        .merge(pa_observability::router(observability))
        .nest("/admin", admin::routes())
        .nest("/market", market::routes())
        .nest("/analysis", analysis::routes())
        .nest("/user", user::routes())
        .layer(middleware::from_fn(track_metrics))
        .with_state(state)
}
```

- [ ] **Step 4: Update `signals.md`**

Replace the `## API` section:

```markdown
## API

| Metric | Type | Labels |
|---|---|---|
| `http_requests_total` | counter | `route`, `method`, `status` |
| `http_request_duration_seconds` | histogram | `route` |
```

- [ ] **Step 5: Run tests**

Run: `cargo test --workspace`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pa-observability/ crates/pa-api/ docs/observability/signals.md
git commit -m "feat(observability): http request middleware"
```

---

## Task 11: Domain — Infrastructure (pg pool gauges)

**Files:**
- Create: `crates/pa-observability/src/domain/infra.rs`
- Modify: `crates/pa-observability/src/domain/mod.rs`
- Modify: `crates/pa-app/src/...` (background gauge updater)
- Modify: `docs/observability/signals.md`

- [ ] **Step 1: Implement infra domain API**

Create `crates/pa-observability/src/domain/infra.rs`:

```rust
use metrics::{describe_gauge, gauge};

const PG_POOL_CONNECTIONS: &str = "pg_pool_connections";

pub fn describe_all() {
    describe_gauge!(PG_POOL_CONNECTIONS, "PostgreSQL pool connection state count.");
}

#[derive(Debug, Clone, Copy)]
pub enum PoolState {
    Idle,
    Active,
}

impl PoolState {
    fn as_label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Active => "active",
        }
    }
}

pub fn pool_connections(state: PoolState, n: u32) {
    gauge!(PG_POOL_CONNECTIONS, "state" => state.as_label()).set(n as f64);
}

pub const METRIC_NAMES: &[&str] = &[PG_POOL_CONNECTIONS];
```

- [ ] **Step 2: Update `domain/mod.rs`**

```rust
pub mod api;
pub mod infra;
pub mod llm;
pub mod market;
pub mod orchestration;

pub fn describe_all() {
    api::describe_all();
    infra::describe_all();
    llm::describe_all();
    market::describe_all();
    orchestration::describe_all();
}
```

- [ ] **Step 3: Spawn a sampler in `pa-app` startup**

In `pa-app/src/main.rs` after the Pg pool is built, spawn:

```rust
use pa_observability::domain::infra as obs_infra;

let pool_for_metrics = pg_pool.clone();
tokio::spawn(async move {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        ticker.tick().await;
        let total = pool_for_metrics.size();
        let idle = pool_for_metrics.num_idle() as u32;
        obs_infra::pool_connections(obs_infra::PoolState::Idle, idle);
        obs_infra::pool_connections(
            obs_infra::PoolState::Active,
            total.saturating_sub(idle),
        );
    }
});
```

- [ ] **Step 4: Update `signals.md`**

Replace the `## Infrastructure` section:

```markdown
## Infrastructure

| Metric | Type | Labels |
|---|---|---|
| `pg_pool_connections` | gauge | `state` |
```

- [ ] **Step 5: Run tests**

Run: `cargo test --workspace`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pa-observability/ crates/pa-app/ docs/observability/signals.md
git commit -m "feat(observability): pg pool gauges"
```

---

## Task 12: OTLP layer (conditional install)

**Files:**
- Modify: `crates/pa-observability/src/init.rs`

- [ ] **Step 1: Replace `init` to conditionally install OTLP**

Replace `init()` body in `crates/pa-observability/src/init.rs`:

```rust
pub fn init(config: ObservabilityConfig) -> anyhow::Result<Guard> {
    let env_filter = EnvFilter::try_new(&config.rust_log)
        .with_context(|| format!("invalid RUST_LOG filter: {}", config.rust_log))?;

    let mut otlp_installed = false;

    let registry = tracing_subscriber::registry().with(env_filter);

    let registry = match config.log_format {
        LogFormat::Json => registry.with(tracing_subscriber::fmt::layer().json().boxed()),
        LogFormat::Text => registry.with(tracing_subscriber::fmt::layer().boxed()),
    };

    if let Some(endpoint) = &config.otlp_endpoint {
        use opentelemetry::trace::TracerProvider as _;
        use opentelemetry_otlp::WithExportConfig;
        use opentelemetry_sdk::Resource;

        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .context("build OTLP span exporter")?;
        let provider = opentelemetry_sdk::trace::TracerProvider::builder()
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .with_resource(Resource::new(vec![opentelemetry::KeyValue::new(
                "service.name",
                config.service_name.clone(),
            )]))
            .build();
        let tracer = provider.tracer(config.service_name.clone());
        opentelemetry::global::set_tracer_provider(provider);
        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        registry
            .with(otel_layer)
            .try_init()
            .map_err(|e| anyhow::anyhow!("tracing init failed: {e}"))?;
        otlp_installed = true;
    } else {
        registry
            .try_init()
            .map_err(|e| anyhow::anyhow!("tracing init failed: {e}"))?;
    }

    let prometheus = PrometheusBuilder::new()
        .install_recorder()
        .context("install prometheus recorder")?;

    crate::domain::describe_all();

    tracing::info!(
        target: "pa_observability",
        metrics = "on",
        otlp = if otlp_installed { "on" } else { "off" },
        service = %config.service_name,
        "observability initialized"
    );

    Ok(Guard {
        prometheus,
        otlp_installed,
    })
}
```

You'll need `tracing_subscriber::Layer::boxed` to make the two arms type-compatible — that requires `tracing_subscriber` feature `"registry"` (already implied by default features). If `boxed()` isn't available, use `Box::new(layer) as Box<dyn Layer<_> + Send + Sync>`.

- [ ] **Step 2: Verify tests pass**

Run: `cargo test --workspace`
Expected: pass. (No automated test for OTLP wire export — manual validation in Task 14.)

- [ ] **Step 3: Audit span names and attributes against spec §6.6**

The spec requires specific span names and attributes:

| Span | Required attributes |
|---|---|
| `api.request` | (existing) — must record `task_id` if it enqueues a task |
| `orchestration.enqueue` | `task_id`, `prompt_key` |
| `orchestration.execute_attempt` | `task_id`, `prompt_key`; root of new trace, OTEL `Link` to enqueue span |
| `llm.call` | `task_id`, `provider`, `model` |
| `schema.validate` | `task_id`, `prompt_key` |
| `repository.save_result` | `task_id` |
| `market.provider.fetch` | `provider` |

For each, locate the existing `tracing::span!` / `#[instrument]` site in
`pa-orchestrator`, `pa-api`, `pa-market` and ensure the span name matches
exactly and required attributes are recorded via `record(...)` or
`#[instrument(fields(...))]`. Where the spec requires an OTEL `Link` from
`orchestration.execute_attempt` back to the enqueue span, capture the
enqueue `SpanContext` at task-record creation time (store as a string in
the persisted task row) and at execute time call:

```rust
use opentelemetry::trace::{SpanContext, TraceContextExt};
use tracing_opentelemetry::OpenTelemetrySpanExt;

let current = tracing::Span::current();
if let Some(parent_ctx) = decode_span_context(&task.enqueue_span_context) {
    current.add_link(parent_ctx);
}
```

If `enqueue_span_context` storage is not yet on the task row, defer the
link wiring to a follow-up — record the gap in `docs/observability/runbook.md`
under "Out-of-Scope" and continue. The plan does not require persisting a
new column in this phase.

- [ ] **Step 4: Manual smoke**

Set `OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317`, run the app
against any OTLP receiver (e.g., the standard
`otel/opentelemetry-collector` Docker image), and verify spans arrive
with the expected names.

- [ ] **Step 5: Commit**

```bash
git add crates/pa-observability/src/init.rs crates/pa-orchestrator/ crates/pa-api/ crates/pa-market/
git commit -m "feat(pa-observability): conditional OTLP trace export + span audit"
```

---

## Task 13: Catalog consistency test

**Files:**
- Create: `crates/pa-observability/tests/catalog_consistency.rs`

- [ ] **Step 1: Add test**

Create `crates/pa-observability/tests/catalog_consistency.rs`:

```rust
//! Enforces that `docs/observability/signals.md` and the `METRIC_NAMES`
//! constants in each domain module agree.

use std::collections::BTreeSet;

const SIGNALS_MD: &str = include_str!("../../../docs/observability/signals.md");

fn metric_names_in_doc() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in SIGNALS_MD.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("| `") {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("| `") {
            if let Some(end) = rest.find('`') {
                let name = &rest[..end];
                // Skip header rows where the cell text is "Metric"
                if name != "Metric" && !name.is_empty() {
                    out.insert(name.to_string());
                }
            }
        }
    }
    out
}

fn metric_names_in_code() -> BTreeSet<String> {
    use pa_observability::domain;
    let mut out = BTreeSet::new();
    for n in domain::api::METRIC_NAMES {
        out.insert((*n).to_string());
    }
    for n in domain::infra::METRIC_NAMES {
        out.insert((*n).to_string());
    }
    for n in domain::llm::METRIC_NAMES {
        out.insert((*n).to_string());
    }
    for n in domain::market::METRIC_NAMES {
        out.insert((*n).to_string());
    }
    for n in domain::orchestration::METRIC_NAMES {
        out.insert((*n).to_string());
    }
    out
}

#[test]
fn doc_and_code_metric_names_match() {
    let doc = metric_names_in_doc();
    let code = metric_names_in_code();
    let only_in_doc: Vec<_> = doc.difference(&code).collect();
    let only_in_code: Vec<_> = code.difference(&doc).collect();
    assert!(
        only_in_doc.is_empty() && only_in_code.is_empty(),
        "signals.md and code disagree.\n  only in signals.md: {only_in_doc:?}\n  only in code: {only_in_code:?}"
    );
}
```

This requires that each `domain/*.rs` exports its `METRIC_NAMES` as `pub`. Tasks 7-11 already define them as `pub const METRIC_NAMES`. Verify they are reachable as `pa_observability::domain::<name>::METRIC_NAMES` (re-export from `domain/mod.rs` is not needed since the modules themselves are `pub mod`).

- [ ] **Step 2: Run test**

Run: `cargo test -p pa-observability --test catalog_consistency`
Expected: passes.

- [ ] **Step 3: Confirm it fails when out of sync**

Temporarily add a line to `signals.md` like ``| `bogus_metric` | counter | — |`` under any section. Re-run:

Expected: test FAILS with message naming `bogus_metric` as "only in signals.md". Remove the line.

- [ ] **Step 4: Commit**

```bash
git add crates/pa-observability/tests/catalog_consistency.rs
git commit -m "test(pa-observability): catalog ↔ code consistency"
```

---

## Task 14: Operator runbook + final validation

**Files:**
- Create: `docs/observability/runbook.md`

- [ ] **Step 1: Create runbook**

Create `docs/observability/runbook.md`:

```markdown
# Observability Runbook

This runbook defines the validation procedure for the observability
foundation and the operational diagnostic flows for common alerts.

## Validation Checklist

Run this once after any change to `pa-observability` initialization or
HTTP routing.

### 1. Default startup (OTLP disabled)

```sh
unset OTEL_EXPORTER_OTLP_ENDPOINT
cargo run -p pa-app
```

Expected log line:

```
observability initialized metrics=on otlp=off service=oh-paa
```

No warnings about OTLP. Process stays running.

### 2. Liveness and readiness

```sh
curl -i http://127.0.0.1:8080/healthz
curl -i http://127.0.0.1:8080/readyz
```

Expected: both return 200. `/readyz` body is `{"status":"ready"}`.

### 3. Readiness reflects dependency loss

Stop PostgreSQL. `curl /readyz` must return 503 with a body listing `pg`
in the failed checks. Restart Pg; `/readyz` returns 200 within one tick
of the health sampler.

### 4. End-to-end metric coverage

```sh
cargo run -p pa-app --bin replay_analysis -- <fixture>
curl -s http://127.0.0.1:8080/metrics > /tmp/metrics.txt
```

The output must contain non-zero values for at least:

- `orchestration_task_duration_seconds_count`
- `orchestration_tasks_total{status="completed"}`
- `llm_request_duration_seconds_count`
- `llm_tokens_total`
- `http_requests_total`

### 5. OTLP export

```sh
docker run -p 4317:4317 -p 4318:4318 otel/opentelemetry-collector
export OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317
cargo run -p pa-app
# trigger one task via API; observe the collector logs/UI for span arrival.
```

Save a screenshot of received spans to `docs/observability/otlp-evidence-YYYY-MM-DD.png`.

### 6. CI gate

```sh
cargo test --workspace
```

Must pass. The catalog-consistency test enforces signals.md ↔ code parity.

## Diagnostic Flows

### Symptom: `orchestration_queue_depth{state="claimed"}` keeps rising

- Check `orchestration_claim_duration_seconds` — high values indicate Pg row-lock contention.
- Check `pg_pool_connections{state="active"}` against pool max.
- Inspect `orchestration_dead_letter_total{reason}` for new failures.

### Symptom: `llm_schema_validation_total{outcome="fail"}` rate climbs

- Pull recent failing prompt_keys from logs (`trace_id` correlation).
- Confirm prompt registry version against last green deploy.
- Compare against `llm_retry_total{reason="schema"}` to confirm whether retries are succeeding.

### Symptom: `/readyz` flapping

- Inspect each failed check name in the response body.
- Default Pg check timeout is 500ms; a flap indicates intermittent Pg latency or pool exhaustion, not Pg downtime.

## Out-of-Scope (For Now)

- Alert rules and SLO definitions (next phase)
- Dashboard JSON (operator side)
- Trace tail-sampling configuration
```

- [ ] **Step 2: Execute the runbook end-to-end**

Run each step in `## Validation Checklist`. Fix any issues that surface.
Save the OTLP screenshot under `docs/observability/`.

- [ ] **Step 3: Final workspace check**

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

All three must succeed. Existing 204 tests + new tests all pass.

- [ ] **Step 4: Commit**

```bash
git add docs/observability/
git commit -m "docs(observability): operator runbook + validation evidence"
```

---

## Closing notes

- Each task's commit is independently revertable.
- After all 14 tasks: `pa-observability` exposes 16 metrics across 5 domains, 3 health-aware HTTP endpoints, structured JSON logs, and optional OTLP traces. Business code references metrics through typed enums; metric names are private constants. The catalog-consistency test guards against drift.
- Next sub-project (B: persistence equivalence + orchestration stress) will reuse this foundation to validate Pg vs InMem orchestration repository behavior under concurrent claim load.
