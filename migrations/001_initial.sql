CREATE TABLE markets (
    id UUID PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    timezone TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE instruments (
    id UUID PRIMARY KEY,
    market_id UUID NOT NULL REFERENCES markets (id) ON DELETE RESTRICT,
    symbol TEXT NOT NULL,
    name TEXT NOT NULL,
    instrument_type TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT instruments_market_symbol_unique UNIQUE (market_id, symbol)
);

CREATE TABLE instrument_symbol_bindings (
    id UUID PRIMARY KEY,
    instrument_id UUID NOT NULL REFERENCES instruments (id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    provider_symbol TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT instrument_symbol_bindings_unique UNIQUE (instrument_id, provider)
);

CREATE TABLE provider_policies (
    id UUID PRIMARY KEY,
    scope_type TEXT NOT NULL CHECK (scope_type IN ('market', 'instrument')),
    market_id UUID REFERENCES markets (id) ON DELETE CASCADE,
    instrument_id UUID REFERENCES instruments (id) ON DELETE CASCADE,
    kline_primary TEXT NOT NULL,
    kline_fallback TEXT,
    tick_primary TEXT NOT NULL,
    tick_fallback TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT provider_policies_scope_match CHECK (
        (
            scope_type = 'market'
            AND market_id IS NOT NULL
            AND instrument_id IS NULL
        )
        OR (
            scope_type = 'instrument'
            AND market_id IS NULL
            AND instrument_id IS NOT NULL
        )
    ),
    CONSTRAINT provider_policies_market_unique UNIQUE (market_id),
    CONSTRAINT provider_policies_instrument_unique UNIQUE (instrument_id)
);

CREATE TABLE canonical_klines (
    id UUID PRIMARY KEY,
    instrument_id UUID NOT NULL REFERENCES instruments (id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    timeframe TEXT NOT NULL,
    open_time TIMESTAMPTZ NOT NULL,
    open NUMERIC(20, 8) NOT NULL,
    high NUMERIC(20, 8) NOT NULL,
    low NUMERIC(20, 8) NOT NULL,
    close NUMERIC(20, 8) NOT NULL,
    volume NUMERIC(28, 8),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT canonical_klines_unique UNIQUE (instrument_id, timeframe, open_time)
);
