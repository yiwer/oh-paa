# Provider / DB E2E Test Plan

## Goal

Verify the end-to-end path from:

1. instrument + provider policy resolution in PostgreSQL
2. provider HTTP fetch
3. canonical K-line storage
4. aggregated K-line read
5. API display surface

## Required Local Inputs

- PostgreSQL database: `postgres://postgres:pgsql@localhost:5432/oh_paa`
- TwelveData key in local `config.toml`
- Runtime config file in repo root:

```toml
database_url = "postgres://postgres:pgsql@localhost:5432/oh_paa"
server_addr = "127.0.0.1:3011"
eastmoney_base_url = "https://push2his.eastmoney.com/"
twelvedata_base_url = "https://api.twelvedata.com/"
twelvedata_api_key = "<real-key>"
```

## Verified Commands

### Quality Gate

```powershell
$env:PA_DATABASE_URL='postgres://postgres:pgsql@localhost:5432/oh_paa'
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Observed on 2026-04-22 Asia/Shanghai:

- `cargo clippy` passed
- `cargo test --workspace` passed

### Live Runtime

Start the app from the repo root:

```powershell
cargo run -p pa-app
```

### Seed Minimal Market Fixtures

Use PostgreSQL to seed at least:

- one crypto instrument with `twelvedata` binding
- one A-share instrument with `eastmoney` binding
- matching market-level provider policies

The live verification in this workspace used:

- instrument `22222222-2222-2222-2222-222222222202` -> `BTC/USD` -> `twelvedata`
- instrument `11111111-1111-1111-1111-111111111101` -> `000001` -> `eastmoney`

### Live Backfill

```powershell
Invoke-WebRequest -UseBasicParsing 'http://127.0.0.1:3011/admin/market/backfill' `
  -Method POST `
  -ContentType 'application/json' `
  -Body '{"instrument_id":"22222222-2222-2222-2222-222222222202","timeframe":"15m","limit":8}'

Invoke-WebRequest -UseBasicParsing 'http://127.0.0.1:3011/admin/market/backfill' `
  -Method POST `
  -ContentType 'application/json' `
  -Body '{"instrument_id":"11111111-1111-1111-1111-111111111101","timeframe":"15m","limit":8}'
```

Observed on 2026-04-22 Asia/Shanghai:

- TwelveData path returned `status=accepted`
- EastMoney path returned `status=accepted`

### Canonical Read Verification

```powershell
Invoke-WebRequest -UseBasicParsing 'http://127.0.0.1:3011/market/canonical?instrument_id=22222222-2222-2222-2222-222222222202&timeframe=15m&limit=8&descending=true'

Invoke-WebRequest -UseBasicParsing 'http://127.0.0.1:3011/market/canonical?instrument_id=11111111-1111-1111-1111-111111111101&timeframe=15m&limit=8&descending=true'
```

Observed on 2026-04-22 Asia/Shanghai:

- TwelveData canonical rows were persisted with `source_provider=twelvedata`
- EastMoney canonical rows were persisted with `source_provider=eastmoney`

### Aggregated Read Verification

```powershell
Invoke-WebRequest -UseBasicParsing 'http://127.0.0.1:3011/market/aggregated?instrument_id=22222222-2222-2222-2222-222222222202&source_timeframe=15m&target_timeframe=1h&limit=4'

Invoke-WebRequest -UseBasicParsing 'http://127.0.0.1:3011/market/aggregated?instrument_id=11111111-1111-1111-1111-111111111101&source_timeframe=15m&target_timeframe=1h&limit=4'
```

Observed on 2026-04-22 Asia/Shanghai:

- Both instruments returned aggregated rows
- Both responses contained `complete=true` buckets when a full 4x15m hour existed
- Incomplete buckets were kept and explicitly marked `complete=false`

## Findings From Live Verification

- TwelveData `time_series` returned naive `YYYY-MM-DD HH:MM:SS` datetimes even when queried with `timezone=UTC`; the provider parser now accepts both RFC3339 and naive formats.
- TwelveData quote/tick payloads can contain integer timestamps; the provider parser now accepts either integer timestamps or string datetimes.
- EastMoney `kline/get` requires extra query parameters such as `fields1`, `fields2`, `fqt`, `beg`, and `end`; omitting them returns `rc=102` with `data=null`.
- EastMoney quote payloads use field aliases like `f43`, `f47`, `f59`, and `f86`; the provider parser now accepts both the live field set and the simpler local fixture shape.
- EastMoney returns naive China-market timestamps; the provider now interprets them as `Asia/Shanghai` local time and stores UTC-normalized bar boundaries.
- Aggregation is still duration-based rather than exchange-session-aware, so partial session buckets are surfaced as `complete=false` instead of being over-asserted as valid higher timeframe bars.

## Remaining Gaps

- Tick query / subscription is not yet exposed through API endpoints in this runtime slice.
- Aggregation is duration-based and gap-aware, but not yet session-calendar-aware. It safely marks partial buckets as incomplete rather than pretending they are fully valid market-session bars.
- Analysis orchestration storage is still in-memory while market data is already PostgreSQL-backed.
