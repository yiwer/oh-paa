# Debug Client: Foundation + Pipeline View — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the backend WebSocket infrastructure, scaffold the React frontend with design system, and implement the Pipeline view — a complete vertical slice from event emission to UI rendering.

**Architecture:** Backend adds a `tokio::sync::broadcast` channel to `AppState` and a `/ws` Axum endpoint that streams `DebugEvent` JSON. Frontend is a React 19 + Vite 6 SPA in `web/`, using styled-components with socket-everyday's neo-brutalist design tokens. Pipeline view consumes both REST (historical) and WebSocket (realtime) data.

**Tech Stack:** Rust (axum 0.8, tokio, serde) · React 19 · Vite 6 · TypeScript · styled-components 6 · Zustand 5 · TanStack React Query 5 · ECharts 5

**Sub-project scope:** This is Plan 1 of 3. Plan 2 covers K-Line Charts, Plan 3 covers LLM Trace + Cross-View Navigation.

---

## File Map

### Backend (Rust) — new/modified files

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `crates/pa-api/src/debug_event.rs` | `DebugEvent` enum + serialization |
| Create | `crates/pa-api/src/ws.rs` | WebSocket upgrade handler + broadcast → client relay |
| Modify | `crates/pa-api/src/router.rs` | Add `broadcast::Sender<DebugEvent>` to `AppState`, mount `/ws` route |
| Modify | `crates/pa-api/Cargo.toml` | Add `tokio` dependency (for broadcast) |
| Modify | `crates/pa-market/src/gateway.rs` | Emit `KlineIngested`, `ProviderFallback` events |
| Modify | `crates/pa-market/Cargo.toml` | Add `tokio` dependency (for broadcast Sender) |
| Modify | `crates/pa-orchestrator/src/worker.rs` | Emit `TaskStatusChanged`, `AttemptCompleted` events |
| Modify | `crates/pa-app/src/main.rs` | Create broadcast channel, inject into AppState and workers |

### Frontend — new files

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `web/package.json` | Dependencies and scripts |
| Create | `web/vite.config.ts` | Dev server, API proxy, build config |
| Create | `web/tsconfig.json` | TypeScript config with `@/` alias |
| Create | `web/index.html` | HTML entry point |
| Create | `web/src/main.tsx` | React root mount |
| Create | `web/src/App.tsx` | Router + QueryClient provider |
| Create | `web/src/theme/tokens.ts` | Color, font, spacing, border tokens |
| Create | `web/src/theme/global.css` | Reset, dot-grid background, typography |
| Create | `web/src/theme/fonts.css` | JetBrains Mono @font-face |
| Create | `web/src/theme/index.ts` | Re-export all tokens |
| Create | `web/src/api/client.ts` | HTTP fetch wrapper |
| Create | `web/src/api/types.ts` | TypeScript interfaces mirroring Rust types |
| Create | `web/src/ws/client.ts` | WebSocket client with reconnect |
| Create | `web/src/ws/useWebSocket.ts` | React hook to connect WS on mount |
| Create | `web/src/ws/debugEventStore.ts` | Zustand store for realtime events |
| Create | `web/src/layout/AppShell.tsx` | Sidebar + Outlet layout |
| Create | `web/src/components/Sidebar/Sidebar.tsx` | Collapsible sidebar navigation |
| Create | `web/src/components/MetricCard/MetricCard.tsx` | Accent-bordered stat card |
| Create | `web/src/components/InstrumentSwitcher/InstrumentSwitcher.tsx` | Market-grouped pill buttons |
| Create | `web/src/pages/PipelinePage.tsx` | Pipeline view composition |
| Create | `web/src/pages/pipeline/InstrumentCard.tsx` | Collapsible instrument status card |
| Create | `web/src/pages/pipeline/EventStream.tsx` | Realtime global event log |
| Create | `web/src/api/hooks/usePipeline.ts` | React Query hooks for pipeline data |

---

## Task 1: Backend — DebugEvent Type

**Files:**
- Create: `crates/pa-api/src/debug_event.rs`

- [ ] **Step 1: Create the DebugEvent enum**

```rust
// crates/pa-api/src/debug_event.rs
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DebugEvent {
    KlineIngested {
        instrument_id: Uuid,
        timeframe: String,
        open_time: DateTime<Utc>,
        provider: String,
        latency_ms: u64,
    },
    ProviderFallback {
        instrument_id: Uuid,
        primary_provider: String,
        fallback_provider: String,
        error: String,
    },
    NormalizationResult {
        instrument_id: Uuid,
        timeframe: String,
        open_time: DateTime<Utc>,
        success: bool,
        error: Option<String>,
    },
    TaskStatusChanged {
        task_id: Uuid,
        instrument_id: Uuid,
        task_type: String,
        old_status: String,
        new_status: String,
    },
    AttemptCompleted {
        task_id: Uuid,
        attempt_number: i32,
        provider: String,
        model: String,
        latency_ms: u64,
        success: bool,
        error: Option<String>,
    },
    OpenBarUpdate {
        instrument_id: Uuid,
        timeframe: String,
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
    },
}
```

- [ ] **Step 2: Register the module in lib.rs**

Add to `crates/pa-api/src/lib.rs`:
```rust
pub mod debug_event;
```
Re-export in the public API so other crates can use it:
```rust
pub use debug_event::DebugEvent;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p pa-api`
Expected: compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/pa-api/src/debug_event.rs crates/pa-api/src/lib.rs
git commit -m "feat(pa-api): add DebugEvent enum for WebSocket streaming"
```

---

## Task 2: Backend — WebSocket Endpoint

**Files:**
- Create: `crates/pa-api/src/ws.rs`
- Modify: `crates/pa-api/src/router.rs`
- Modify: `crates/pa-api/Cargo.toml`

- [ ] **Step 1: Add tokio dependency to pa-api**

In `crates/pa-api/Cargo.toml`, add under `[dependencies]`:
```toml
tokio.workspace = true
```

- [ ] **Step 2: Add broadcast::Sender to AppState**

In `crates/pa-api/src/router.rs`, add the field to `AppState`:
```rust
use tokio::sync::broadcast;
use crate::debug_event::DebugEvent;
```

Add field:
```rust
pub debug_tx: broadcast::Sender<DebugEvent>,
```

Update all constructors (`new`, `with_dependencies`, `with_market_runtime`, `fixture`) to accept or create a `broadcast::Sender<DebugEvent>`. In `fixture()`, create a dummy channel:
```rust
let (debug_tx, _) = broadcast::channel(256);
```

- [ ] **Step 3: Create ws.rs handler**

```rust
// crates/pa-api/src/ws.rs
use axum::{
    extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}},
    response::IntoResponse,
};
use tokio::sync::broadcast;
use crate::{debug_event::DebugEvent, router::AppState};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.debug_tx))
}

async fn handle_socket(
    mut socket: WebSocket,
    debug_tx: broadcast::Sender<DebugEvent>,
) {
    let mut rx = debug_tx.subscribe();

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        let json = serde_json::to_string(&event).unwrap();
                        if socket.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(missed = n, "ws client lagged, skipping events");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
```

- [ ] **Step 4: Register ws module and mount route**

In `crates/pa-api/src/lib.rs`, add:
```rust
pub mod ws;
```

In `crates/pa-api/src/router.rs`, in `app_router()`, add before `.with_state(state)`:
```rust
.route("/ws", get(ws::ws_handler))
```

Add to imports:
```rust
use axum::routing::get;
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p pa-api`
Expected: compiles. Tests may need updating for new AppState constructor — fix any that fail by adding the `debug_tx` field.

- [ ] **Step 6: Fix any broken tests**

Run: `cargo test -p pa-api`
Any test that constructs `AppState` directly needs `debug_tx` added. Use `broadcast::channel(16)` in tests.

- [ ] **Step 7: Commit**

```bash
git add crates/pa-api/src/ws.rs crates/pa-api/src/router.rs crates/pa-api/src/lib.rs crates/pa-api/Cargo.toml
git commit -m "feat(pa-api): add /ws WebSocket endpoint for debug event streaming"
```

---

## Task 3: Backend — Wire Broadcast Channel in pa-app

**Files:**
- Modify: `crates/pa-app/src/main.rs`

- [ ] **Step 1: Create broadcast channel and inject into AppState**

In `main()`, after building `market_runtime` and before building `state`:
```rust
let (debug_tx, _) = tokio::sync::broadcast::channel::<pa_api::DebugEvent>(512);
```

Update the `AppState` construction to include `debug_tx.clone()`.

- [ ] **Step 2: Verify the app starts**

Run: `cargo run -p pa-app`
Expected: app starts and listens. `/ws` endpoint accepts connections (test with `websocat ws://127.0.0.1:3000/ws` if available, or just verify no panic on startup).

- [ ] **Step 3: Commit**

```bash
git add crates/pa-app/src/main.rs
git commit -m "feat(pa-app): wire broadcast channel for debug events"
```

---

## Task 4: Backend — Emit Events from Market Gateway

**Files:**
- Modify: `crates/pa-market/src/gateway.rs`
- Modify: `crates/pa-market/Cargo.toml`

- [ ] **Step 1: Add tokio and pa-api dependency to pa-market**

In `crates/pa-market/Cargo.toml`:
```toml
tokio.workspace = true
```

Note: Rather than making pa-market depend on pa-api (circular dependency risk), define an event sender trait or pass a closure. The simplest approach: add an optional `broadcast::Sender` field directly to `MarketGateway`, with `DebugEvent` defined in pa-api.

However, to avoid circular deps, move `DebugEvent` to `pa-core` instead. Update `crates/pa-core/src/lib.rs` to include `pub mod debug_event;` and move the file there. Then pa-market can depend on pa-core (which it already does).

- [ ] **Step 2: Move DebugEvent to pa-core**

Move `crates/pa-api/src/debug_event.rs` to `crates/pa-core/src/debug_event.rs`.

In `crates/pa-core/src/lib.rs` add:
```rust
pub mod debug_event;
pub use debug_event::DebugEvent;
```

In `crates/pa-core/Cargo.toml`, ensure `rust_decimal` is a dependency (it already is via workspace).

In `crates/pa-api/src/lib.rs`, remove the local `debug_event` module and re-export from pa-core:
```rust
pub use pa_core::DebugEvent;
```

Update `crates/pa-api/src/ws.rs` import to use `pa_core::DebugEvent`.

- [ ] **Step 3: Add broadcast Sender to MarketGateway**

In `crates/pa-market/src/gateway.rs`:
```rust
use tokio::sync::broadcast;
use pa_core::DebugEvent;
```

Add field to `MarketGateway`:
```rust
pub struct MarketGateway {
    router: ProviderRouter,
    debug_tx: Option<broadcast::Sender<DebugEvent>>,
}
```

Add constructor:
```rust
impl MarketGateway {
    pub fn new(router: ProviderRouter) -> Self {
        Self { router, debug_tx: None }
    }

    pub fn with_debug_tx(mut self, tx: broadcast::Sender<DebugEvent>) -> Self {
        self.debug_tx = Some(tx);
        self
    }

    fn emit(&self, event: DebugEvent) {
        if let Some(tx) = &self.debug_tx {
            let _ = tx.send(event); // ignore if no receivers
        }
    }
}
```

- [ ] **Step 4: Emit events in fetch_klines**

In `MarketGateway::fetch_klines`, after a successful fetch, emit:
```rust
self.emit(DebugEvent::KlineIngested {
    instrument_id: ctx.instrument_id,
    timeframe: timeframe.to_string(),
    open_time: klines.klines.first().map(|k| k.open_time).unwrap_or_else(Utc::now),
    provider: klines.provider_name.clone(),
    latency_ms: start.elapsed().as_millis() as u64,
});
```

Add `let start = std::time::Instant::now();` at the top of the method.

For fallback scenarios (when primary fails and fallback is used), also emit `ProviderFallback`. This requires checking whether the `RoutedKlines` came from the fallback provider — compare `klines.provider_name` against the expected primary.

- [ ] **Step 5: Verify compilation and tests**

Run: `cargo check -p pa-market && cargo test -p pa-market`
Expected: passes. Existing tests create `MarketGateway::new(...)` which gets `debug_tx: None` — no events emitted, no breakage.

- [ ] **Step 6: Wire debug_tx in pa-app main**

In `crates/pa-app/src/main.rs`, update the `MarketGateway` construction:
```rust
let market_gateway = Arc::new(
    MarketGateway::new(provider_router).with_debug_tx(debug_tx.clone())
);
```

- [ ] **Step 7: Commit**

```bash
git add crates/pa-core/src/debug_event.rs crates/pa-core/src/lib.rs \
       crates/pa-market/src/gateway.rs crates/pa-market/Cargo.toml \
       crates/pa-api/src/lib.rs crates/pa-api/src/ws.rs \
       crates/pa-app/src/main.rs
git commit -m "feat(pa-market): emit DebugEvent from MarketGateway on kline fetch"
```

---

## Task 5: Frontend — Scaffold Vite Project

**Files:**
- Create: `web/package.json`, `web/vite.config.ts`, `web/tsconfig.json`, `web/index.html`, `web/src/main.tsx`, `web/src/vite-env.d.ts`

- [ ] **Step 1: Create package.json**

```json
{
  "name": "oh-paa-web",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "lint": "tsc --noEmit"
  },
  "dependencies": {
    "@tanstack/react-query": "^5.59.0",
    "echarts": "^5.5.1",
    "echarts-for-react": "^3.0.2",
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "react-router-dom": "^7.0.0",
    "styled-components": "^6.1.13",
    "zustand": "^5.0.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "typescript": "^5.6.0",
    "vite": "^6.0.0"
  }
}
```

- [ ] **Step 2: Create vite.config.ts**

```typescript
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { '@': resolve(__dirname, 'src') },
  },
  server: {
    host: '127.0.0.1',
    port: 5173,
    proxy: {
      '/api/ws': {
        target: 'ws://127.0.0.1:3000',
        ws: true,
        rewrite: (path) => path.replace(/^\/api/, ''),
      },
      '/api': {
        target: 'http://127.0.0.1:3000',
        rewrite: (path) => path.replace(/^\/api/, ''),
      },
    },
  },
  build: {
    target: 'es2022',
  },
});
```

- [ ] **Step 3: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "paths": { "@/*": ["./src/*"] },
    "baseUrl": ".",
    "skipLibCheck": true
  },
  "include": ["src"]
}
```

- [ ] **Step 4: Create index.html and entry point**

`web/index.html`:
```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>oh-paa</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

`web/src/main.tsx`:
```tsx
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
```

`web/src/vite-env.d.ts`:
```typescript
/// <reference types="vite/client" />
```

`web/src/App.tsx`:
```tsx
export default function App() {
  return <div>oh-paa</div>;
}
```

- [ ] **Step 5: Install dependencies and verify dev server starts**

```bash
cd web && npm install && npm run dev
```

Expected: Vite dev server starts at http://127.0.0.1:5173, page shows "oh-paa".

- [ ] **Step 6: Add web/node_modules to .gitignore**

In root `.gitignore`, add:
```
web/node_modules/
web/dist/
```

- [ ] **Step 7: Commit**

```bash
git add web/package.json web/vite.config.ts web/tsconfig.json web/index.html \
       web/src/main.tsx web/src/App.tsx web/src/vite-env.d.ts .gitignore
git commit -m "feat(web): scaffold React + Vite frontend project"
```

---

## Task 6: Frontend — Design System (Tokens + Global CSS)

**Files:**
- Create: `web/src/theme/tokens.ts`, `web/src/theme/global.css`, `web/src/theme/fonts.css`, `web/src/theme/index.ts`

- [ ] **Step 1: Create design tokens**

`web/src/theme/tokens.ts`:
```typescript
export const color = {
  yellowPrimary: '#FFDE00',
  bluePrimary: '#6FC2FF',
  tealAccent: '#53DBC9',
  redAccent: '#FF7169',
  bgBeige: '#F4EFEA',
  bgOffwhite: '#F8F8F7',
  bgWhite: '#FFFFFF',
  bgLightGray: '#F1F1F1',
  textDark: '#383838',
  textGray: '#818181',
  textLightGray: '#A1A1A1',
  darkSurface: '#383838',
} as const;

export const font = {
  mono: '"JetBrains Mono", "PingFang SC", "Microsoft YaHei", monospace',
} as const;

export const size = {
  display: 56,
  h2: 24,
  h3: 14,
  eyebrow: 10,
  bodyLg: 16,
  body: 14,
  bodySm: 13,
  bodyXs: 12,
  caption: 11,
  mini: 10,
} as const;

export const space = {
  px4: 4, px6: 6, px8: 8, px10: 10, px12: 12,
  px16: 16, px20: 20, px24: 24, px32: 32, px48: 48,
} as const;

export const border = {
  thin: `1px solid ${color.textDark}`,
  std: `2px solid ${color.textDark}`,
  thick: `3px solid ${color.textDark}`,
  dashed: `1px dashed ${color.bgLightGray}`,
  dashedSection: `2px dashed ${color.bgLightGray}`,
  radius: '0px',
} as const;

export const transition = {
  btn: 'transform 0.12s ease-in-out',
  card: '0.4s ease-out',
  nav: 'background-color 0.2s ease-in-out',
} as const;
```

- [ ] **Step 2: Create global CSS**

`web/src/theme/global.css`:
```css
@import './fonts.css';

*, *::before, *::after { box-sizing: border-box; }
body {
  margin: 0;
  font-family: "JetBrains Mono", "PingFang SC", "Microsoft YaHei", monospace;
  background: #F4EFEA;
  background-image: radial-gradient(circle at 1px 1px, rgba(56,56,56,0.08) 1px, transparent 0);
  background-size: 24px 24px;
  color: #383838;
  -webkit-font-smoothing: antialiased;
}
h1, h2, h3, h4 { margin: 0; text-transform: uppercase; }
a { color: inherit; text-decoration: none; }
::selection { background: #6FC2FF; color: #383838; }
```

`web/src/theme/fonts.css`:
```css
@font-face {
  font-family: 'JetBrains Mono';
  src: url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;700&display=swap');
}
```

Note: Use Google Fonts link import instead of @font-face in `web/index.html` for simplicity:
```html
<link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;700&display=swap" rel="stylesheet">
```
Remove the `fonts.css` @font-face and just keep the file as an empty placeholder or delete it.

- [ ] **Step 3: Create index re-export**

`web/src/theme/index.ts`:
```typescript
export { color, font, size, space, border, transition } from './tokens';
```

- [ ] **Step 4: Import global CSS in main.tsx**

Update `web/src/main.tsx`:
```tsx
import './theme/global.css';
```

- [ ] **Step 5: Verify styles load in browser**

Run: `npm run dev` (in web/)
Expected: beige dot-grid background, JetBrains Mono font.

- [ ] **Step 6: Commit**

```bash
git add web/src/theme/ web/src/main.tsx web/index.html
git commit -m "feat(web): add neo-brutalist design tokens and global CSS"
```

---

## Task 7: Frontend — AppShell + Sidebar + Router

**Files:**
- Create: `web/src/layout/AppShell.tsx`, `web/src/components/Sidebar/Sidebar.tsx`
- Modify: `web/src/App.tsx`

- [ ] **Step 1: Create Sidebar component**

`web/src/components/Sidebar/Sidebar.tsx`:
```tsx
import { NavLink } from 'react-router-dom';
import styled from 'styled-components';
import { color, font, border, space, transition } from '@/theme';

const navItems = [
  { to: '/pipeline', label: 'Pipeline', icon: 'P' },
  { to: '/kline', label: 'K-Line Charts', icon: 'K' },
  { to: '/llm-trace', label: 'LLM Trace', icon: 'L' },
];

interface Props {
  wsConnected: boolean;
}

export default function Sidebar({ wsConnected }: Props) {
  return (
    <Wrap>
      <Brand>oh-paa</Brand>
      <Nav>
        {navItems.map((item) => (
          <StyledNavLink key={item.to} to={item.to}>
            <IconBox>{item.icon}</IconBox>
            <span>{item.label}</span>
          </StyledNavLink>
        ))}
      </Nav>
      <Footer>
        <WsStatus $connected={wsConnected}>
          ● {wsConnected ? 'Connected' : 'Disconnected'}
        </WsStatus>
      </Footer>
    </Wrap>
  );
}

const Wrap = styled.aside`
  width: 200px;
  min-height: 100vh;
  background: ${color.darkSurface};
  color: ${color.bgBeige};
  display: flex;
  flex-direction: column;
  padding: ${space.px16}px;
  flex-shrink: 0;
`;

const Brand = styled.div`
  font-size: 18px;
  font-weight: 700;
  color: ${color.yellowPrimary};
  letter-spacing: 2px;
  margin-bottom: ${space.px32}px;
  text-transform: uppercase;
`;

const Nav = styled.nav`
  display: flex;
  flex-direction: column;
  gap: ${space.px4}px;
`;

const StyledNavLink = styled(NavLink)`
  display: flex;
  align-items: center;
  gap: ${space.px10}px;
  padding: 8px 12px;
  font-size: 13px;
  font-family: ${font.mono};
  color: ${color.textLightGray};
  transition: ${transition.nav};

  &.active {
    background: ${color.yellowPrimary};
    color: ${color.textDark};
    font-weight: 700;
  }

  &:hover:not(.active) {
    background: rgba(255, 255, 255, 0.05);
  }
`;

const IconBox = styled.span`
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: ${border.std};
  background: ${color.bgWhite};
  color: ${color.textDark};
  font-size: 11px;
  font-weight: 700;
  flex-shrink: 0;
`;

const Footer = styled.div`
  margin-top: auto;
  padding-top: ${space.px16}px;
  border-top: 1px dashed rgba(255,255,255,0.15);
`;

const WsStatus = styled.div<{ $connected: boolean }>`
  font-size: 11px;
  padding: 8px 12px;
  color: ${(p) => (p.$connected ? color.tealAccent : color.redAccent)};
`;
```

- [ ] **Step 2: Create AppShell layout**

`web/src/layout/AppShell.tsx`:
```tsx
import { Outlet } from 'react-router-dom';
import styled from 'styled-components';
import Sidebar from '@/components/Sidebar/Sidebar';
import { color, space } from '@/theme';

export default function AppShell() {
  // TODO: wire real WS status in Task 10
  const wsConnected = false;

  return (
    <Page>
      <Sidebar wsConnected={wsConnected} />
      <Main>
        <Outlet />
      </Main>
    </Page>
  );
}

const Page = styled.div`
  min-height: 100vh;
  display: flex;
  background: ${color.bgBeige};
`;

const Main = styled.main`
  flex: 1;
  padding: 28px 36px 80px;
  min-width: 0;
  max-width: 1440px;
`;
```

- [ ] **Step 3: Set up router in App.tsx**

`web/src/App.tsx`:
```tsx
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import AppShell from '@/layout/AppShell';

const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 30_000, refetchOnWindowFocus: false } },
});

function PlaceholderPage({ title }: { title: string }) {
  return <h2 style={{ fontSize: 24, fontWeight: 700 }}>{title}</h2>;
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route element={<AppShell />}>
            <Route path="/" element={<Navigate to="/pipeline" replace />} />
            <Route path="/pipeline" element={<PlaceholderPage title="Pipeline" />} />
            <Route path="/kline" element={<PlaceholderPage title="K-Line Charts" />} />
            <Route path="/llm-trace" element={<PlaceholderPage title="LLM Trace" />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
```

- [ ] **Step 4: Verify in browser**

Run: `npm run dev` (in web/)
Expected: dark sidebar with "oh-paa" brand, three nav links, beige content area. Clicking nav links highlights active item.

- [ ] **Step 5: Commit**

```bash
git add web/src/layout/ web/src/components/Sidebar/ web/src/App.tsx
git commit -m "feat(web): add AppShell layout with sidebar navigation"
```

---

## Task 8: Frontend — API Client + TypeScript Types

**Files:**
- Create: `web/src/api/client.ts`, `web/src/api/types.ts`

- [ ] **Step 1: Create HTTP client**

`web/src/api/client.ts`:
```typescript
const BASE = '/api';

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
  ) {
    super(message);
  }
}

export async function api<T = unknown>(
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    ...init,
    headers: {
      'content-type': 'application/json',
      ...init.headers,
    },
  });

  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: { code: 'unknown', message: res.statusText } }));
    throw new ApiError(res.status, body.error?.code ?? 'unknown', body.error?.message ?? res.statusText);
  }

  if (res.status === 204) return undefined as T;
  return res.json();
}
```

- [ ] **Step 2: Create TypeScript types**

`web/src/api/types.ts`:
```typescript
export interface Market {
  id: string;
  code: string;
  name: string;
  timezone: string;
}

export interface Instrument {
  id: string;
  market_id: string;
  symbol: string;
  name: string;
  instrument_type: string;
}

export interface CanonicalKline {
  instrument_id: string;
  timeframe: string;
  open_time: string;
  close_time: string;
  open: string;
  high: string;
  low: string;
  close: string;
  volume: string;
  source_provider: string;
}

export interface AggregatedKline {
  open_time: string;
  close_time: string;
  open: string;
  high: string;
  low: string;
  close: string;
  volume: string;
  complete: boolean;
  child_bar_count: number;
  expected_child_bar_count: number;
}

export interface AnalysisTask {
  id: string;
  snapshot_id: string;
  task_type: string;
  status: string;
  instrument_id: string;
  timeframe: string | null;
  bar_state: string;
  bar_open_time: string | null;
  bar_close_time: string | null;
  prompt_key: string;
  prompt_version: string;
  attempt_count: number;
  max_attempts: number;
  started_at: string | null;
  finished_at: string | null;
  last_error_code: string | null;
  last_error_message: string | null;
}

export interface AnalysisAttempt {
  id: string;
  task_id: string;
  attempt_number: number;
  worker_id: string;
  llm_provider: string;
  model: string;
  request_payload_json: Record<string, unknown>;
  raw_response_json: Record<string, unknown> | null;
  parsed_output_json: Record<string, unknown> | null;
  error_type: string | null;
  error_message: string | null;
  started_at: string;
  finished_at: string;
}

export interface AnalysisResult {
  id: string;
  task_id: string;
  output_json: Record<string, unknown>;
  created_at: string;
}

export interface AnalysisDeadLetter {
  id: string;
  task_id: string;
  archived_snapshot_json: Record<string, unknown>;
  final_error_type: string;
  final_error_message: string;
  last_attempt_id: string;
  created_at: string;
}

export type DebugEventType =
  | 'kline_ingested'
  | 'provider_fallback'
  | 'normalization_result'
  | 'task_status_changed'
  | 'attempt_completed'
  | 'open_bar_update';

export interface DebugEvent {
  type: DebugEventType;
  [key: string]: unknown;
}

export interface SessionProfile {
  market_code: string;
  market_timezone: string;
  session_kind: string;
}
```

- [ ] **Step 3: Commit**

```bash
git add web/src/api/
git commit -m "feat(web): add API client and TypeScript type definitions"
```

---

## Task 9: Frontend — MetricCard Component

**Files:**
- Create: `web/src/components/MetricCard/MetricCard.tsx`

- [ ] **Step 1: Create MetricCard**

`web/src/components/MetricCard/MetricCard.tsx`:
```tsx
import styled, { keyframes } from 'styled-components';
import { color, border, space, font } from '@/theme';

export type MetricAccent = 'teal' | 'blue' | 'yellow' | 'red' | 'gray';

const accentColors: Record<MetricAccent, string> = {
  teal: color.tealAccent,
  blue: color.bluePrimary,
  yellow: color.yellowPrimary,
  red: color.redAccent,
  gray: color.textGray,
};

interface Props {
  accent?: MetricAccent;
  eyebrow: string;
  value: React.ReactNode;
  sub?: string;
}

export default function MetricCard({ accent = 'gray', eyebrow, value, sub }: Props) {
  return (
    <Card $accent={accent}>
      <Eyebrow>{eyebrow}</Eyebrow>
      <Value>{value}</Value>
      {sub && <Sub>{sub}</Sub>}
    </Card>
  );
}

export function MetricStrip({ children, ...rest }: React.HTMLAttributes<HTMLDivElement>) {
  return <Strip {...rest}>{children}</Strip>;
}

const fadeIn = keyframes`
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
`;

const Strip = styled.div`
  display: flex;
  gap: ${space.px10}px;
  margin-bottom: ${space.px16}px;

  & > * {
    animation: ${fadeIn} 0.3s ease-out both;
  }
  ${Array.from({ length: 6 }, (_, i) => `& > *:nth-child(${i + 1}) { animation-delay: ${i * 60}ms; }`).join('\n')}
`;

const Card = styled.div<{ $accent: MetricAccent }>`
  flex: 1;
  background: ${color.bgWhite};
  border: ${border.std};
  border-left: 4px solid ${(p) => accentColors[p.$accent]};
  padding: ${space.px10}px ${space.px12}px;
  font-family: ${font.mono};
`;

const Eyebrow = styled.div`
  font-size: 10px;
  text-transform: uppercase;
  color: ${color.textGray};
  letter-spacing: 1px;
`;

const Value = styled.div`
  font-size: 22px;
  font-weight: 700;
  margin-top: 2px;
`;

const Sub = styled.div`
  font-size: 11px;
  color: ${color.textGray};
  margin-top: 2px;
`;
```

- [ ] **Step 2: Verify in browser**

Temporarily render a MetricStrip in the Pipeline placeholder to check styling.

- [ ] **Step 3: Commit**

```bash
git add web/src/components/MetricCard/
git commit -m "feat(web): add MetricCard component with staggered animation"
```

---

## Task 10: Frontend — WebSocket Client + Debug Event Store

**Files:**
- Create: `web/src/ws/client.ts`, `web/src/ws/useWebSocket.ts`, `web/src/ws/debugEventStore.ts`

- [ ] **Step 1: Create Zustand event store**

`web/src/ws/debugEventStore.ts`:
```typescript
import { create } from 'zustand';
import type { DebugEvent } from '@/api/types';

interface DebugEventState {
  events: DebugEvent[];
  connected: boolean;
  setConnected: (v: boolean) => void;
  push: (event: DebugEvent) => void;
  clear: () => void;
}

const MAX_EVENTS = 200;

export const useDebugEventStore = create<DebugEventState>((set) => ({
  events: [],
  connected: false,
  setConnected: (connected) => set({ connected }),
  push: (event) =>
    set((state) => ({
      events: [...state.events, event].slice(-MAX_EVENTS),
    })),
  clear: () => set({ events: [] }),
}));
```

- [ ] **Step 2: Create WebSocket client**

`web/src/ws/client.ts`:
```typescript
import type { DebugEvent } from '@/api/types';
import { useDebugEventStore } from './debugEventStore';

let ws: WebSocket | null = null;
let retry = 0;
let retryTimer: ReturnType<typeof setTimeout> | null = null;
let closedManually = false;

export function connectWs() {
  closedManually = false;
  const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const url = `${proto}//${window.location.host}/api/ws`;

  ws = new WebSocket(url);

  ws.onopen = () => {
    retry = 0;
    useDebugEventStore.getState().setConnected(true);
  };

  ws.onmessage = (ev) => {
    try {
      const event: DebugEvent = JSON.parse(ev.data);
      useDebugEventStore.getState().push(event);
    } catch {
      // ignore malformed messages
    }
  };

  ws.onclose = () => {
    useDebugEventStore.getState().setConnected(false);
    if (closedManually) return;
    const delay = Math.min(30_000, 1000 * 2 ** retry);
    retry++;
    retryTimer = setTimeout(connectWs, delay);
  };

  ws.onerror = () => {
    ws?.close();
  };
}

export function disconnectWs() {
  closedManually = true;
  if (retryTimer) clearTimeout(retryTimer);
  ws?.close();
  ws = null;
}
```

- [ ] **Step 3: Create useWebSocket hook**

`web/src/ws/useWebSocket.ts`:
```typescript
import { useEffect } from 'react';
import { connectWs, disconnectWs } from './client';

export function useWebSocket() {
  useEffect(() => {
    connectWs();
    return () => disconnectWs();
  }, []);
}
```

- [ ] **Step 4: Wire into AppShell**

In `web/src/layout/AppShell.tsx`, import and use:
```tsx
import { useWebSocket } from '@/ws/useWebSocket';
import { useDebugEventStore } from '@/ws/debugEventStore';

export default function AppShell() {
  useWebSocket();
  const wsConnected = useDebugEventStore((s) => s.connected);
  // ... rest unchanged
}
```

- [ ] **Step 5: Verify WS connection in browser**

Start both backend (`cargo run -p pa-app`) and frontend (`npm run dev`).
Expected: Sidebar shows "● Connected" in teal when backend is running, "● Disconnected" in red when backend is stopped.

- [ ] **Step 6: Commit**

```bash
git add web/src/ws/ web/src/layout/AppShell.tsx
git commit -m "feat(web): add WebSocket client with auto-reconnect and debug event store"
```

---

## Task 11: Frontend — Pipeline Page (Stats + Instrument Cards)

**Files:**
- Create: `web/src/pages/PipelinePage.tsx`, `web/src/pages/pipeline/InstrumentCard.tsx`, `web/src/api/hooks/usePipeline.ts`
- Modify: `web/src/App.tsx`

- [ ] **Step 1: Create pipeline data hook**

`web/src/api/hooks/usePipeline.ts`:
```typescript
import { useQuery } from '@tanstack/react-query';
import { api } from '@/api/client';
import type { Instrument, CanonicalKline, SessionProfile } from '@/api/types';

export function useInstruments() {
  return useQuery({
    queryKey: ['instruments'],
    queryFn: () => api<{ rows: Instrument[] }>('/market/instruments').then((r) => r.rows),
    staleTime: 60_000,
  });
}

export function useCanonicalKlines(instrumentId: string, timeframe: string) {
  return useQuery({
    queryKey: ['canonical-klines', instrumentId, timeframe],
    queryFn: () =>
      api<{ rows: CanonicalKline[] }>(
        `/market/canonical?instrument_id=${instrumentId}&timeframe=${timeframe}&limit=10&descending=true`,
      ).then((r) => r.rows),
    enabled: !!instrumentId,
    staleTime: 15_000,
  });
}

export function useSessionProfile(instrumentId: string) {
  return useQuery({
    queryKey: ['session-profile', instrumentId],
    queryFn: () => api<SessionProfile>(`/market/session-profile?instrument_id=${instrumentId}`),
    enabled: !!instrumentId,
    staleTime: 300_000,
  });
}
```

- [ ] **Step 2: Create InstrumentCard component**

`web/src/pages/pipeline/InstrumentCard.tsx`:
```tsx
import { useState } from 'react';
import styled from 'styled-components';
import { color, border, space, font } from '@/theme';
import type { Instrument, DebugEvent } from '@/api/types';

interface Props {
  instrument: Instrument;
  events: DebugEvent[];
  hasError: boolean;
}

export default function InstrumentCard({ instrument, events, hasError }: Props) {
  const [expanded, setExpanded] = useState(false);

  const recentEvents = events
    .filter((e) => (e as Record<string, unknown>).instrument_id === instrument.id)
    .slice(-5);

  return (
    <Card $hasError={hasError}>
      <Header onClick={() => setExpanded(!expanded)}>
        <Left>
          <Symbol>{instrument.symbol}</Symbol>
          <Name>{instrument.name}</Name>
        </Left>
        <Right>
          <Arrow>{expanded ? '▾' : '▸'}</Arrow>
        </Right>
      </Header>

      {expanded && (
        <Detail>
          <SectionTitle>Recent Events</SectionTitle>
          {recentEvents.length === 0 && <Empty>No events yet</Empty>}
          {recentEvents.map((ev, i) => (
            <EventRow key={i}>
              <EventType>{ev.type}</EventType>
            </EventRow>
          ))}
        </Detail>
      )}
    </Card>
  );
}

const Card = styled.div<{ $hasError: boolean }>`
  background: ${color.bgWhite};
  border: 2px solid ${(p) => (p.$hasError ? color.redAccent : color.textDark)};
  margin-bottom: ${space.px8}px;
  font-family: ${font.mono};
`;

const Header = styled.div`
  padding: ${space.px12}px ${space.px16}px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  cursor: pointer;

  &:hover { background: ${color.bgOffwhite}; }
`;

const Left = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px12}px;
`;

const Symbol = styled.span`
  font-weight: 700;
  font-size: 14px;
`;

const Name = styled.span`
  font-size: 11px;
  color: ${color.textGray};
`;

const Right = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px16}px;
  font-size: 11px;
`;

const Arrow = styled.span`
  font-size: 14px;
`;

const Detail = styled.div`
  border-top: 2px dashed ${color.bgLightGray};
  padding: ${space.px12}px ${space.px16}px;
`;

const SectionTitle = styled.div`
  font-size: 11px;
  font-weight: 700;
  margin-bottom: ${space.px8}px;
`;

const Empty = styled.div`
  font-size: 11px;
  color: ${color.textLightGray};
`;

const EventRow = styled.div`
  padding: 3px 0;
  border-bottom: ${border.dashed};
  font-size: 11px;
`;

const EventType = styled.span`
  color: ${color.textGray};
`;
```

- [ ] **Step 3: Create PipelinePage**

`web/src/pages/PipelinePage.tsx`:
```tsx
import styled from 'styled-components';
import { color, space, font } from '@/theme';
import MetricCard, { MetricStrip } from '@/components/MetricCard/MetricCard';
import InstrumentCard from './pipeline/InstrumentCard';
import { useDebugEventStore } from '@/ws/debugEventStore';
import { useInstruments } from '@/api/hooks/usePipeline';

export default function PipelinePage() {
  const { data: instruments, isLoading } = useInstruments();
  const events = useDebugEventStore((s) => s.events);

  const klineEvents = events.filter((e) => e.type === 'kline_ingested');
  const fallbackEvents = events.filter((e) => e.type === 'provider_fallback');
  const errorEvents = events.filter(
    (e) => e.type === 'normalization_result' && !(e as Record<string, unknown>).success,
  );

  // Group instruments by market (simplified: use symbol pattern)
  const crypto = (instruments ?? []).filter((i) => i.symbol.includes('/USDT'));
  const forex = (instruments ?? []).filter((i) => !i.symbol.includes('/USDT'));

  return (
    <div>
      <PageHeader>
        <Title>Market Data Pipeline</Title>
        <Subtitle>实时数据摄入 & Provider 路由状态</Subtitle>
      </PageHeader>

      <MetricStrip>
        <MetricCard accent="teal" eyebrow="Klines Ingested" value={klineEvents.length} sub="this session" />
        <MetricCard accent="blue" eyebrow="Provider Routes" value={instruments?.length ?? 0} sub={`${fallbackEvents.length} fallback`} />
        <MetricCard accent="yellow" eyebrow="Normalization" value="—" sub="success rate" />
        <MetricCard accent="red" eyebrow="Errors" value={errorEvents.length} sub="this session" />
      </MetricStrip>

      {isLoading && <Loading>Loading instruments...</Loading>}

      {crypto.length > 0 && (
        <>
          <GroupTitle>Crypto — {crypto.length} instruments</GroupTitle>
          {crypto.map((inst) => (
            <InstrumentCard
              key={inst.id}
              instrument={inst}
              events={events}
              hasError={fallbackEvents.some(
                (e) => (e as Record<string, unknown>).instrument_id === inst.id,
              )}
            />
          ))}
        </>
      )}

      {forex.length > 0 && (
        <>
          <GroupTitle>Forex — {forex.length} instruments</GroupTitle>
          {forex.map((inst) => (
            <InstrumentCard
              key={inst.id}
              instrument={inst}
              events={events}
              hasError={false}
            />
          ))}
        </>
      )}
    </div>
  );
}

const PageHeader = styled.div`
  margin-bottom: ${space.px20}px;
`;

const Title = styled.h2`
  font-size: 20px;
  font-weight: 700;
  font-family: ${font.mono};
  text-transform: none;
`;

const Subtitle = styled.div`
  font-size: 12px;
  color: ${color.textGray};
  margin-top: ${space.px4}px;
`;

const GroupTitle = styled.div`
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 2px;
  color: ${color.textGray};
  margin-bottom: ${space.px8}px;
  padding-bottom: 4px;
  border-bottom: 2px solid ${color.textDark};
`;

const Loading = styled.div`
  font-size: 12px;
  color: ${color.textGray};
`;
```

- [ ] **Step 4: Wire PipelinePage into router**

In `web/src/App.tsx`, replace the Pipeline placeholder:
```tsx
import PipelinePage from '@/pages/PipelinePage';
// ...
<Route path="/pipeline" element={<PipelinePage />} />
```

- [ ] **Step 5: Verify in browser**

Run backend + frontend. Navigate to `/pipeline`.
Expected: MetricCard strip renders, instrument list loads from REST API (or shows loading state), WebSocket events populate cards as they arrive.

- [ ] **Step 6: Commit**

```bash
git add web/src/pages/ web/src/api/hooks/ web/src/App.tsx
git commit -m "feat(web): implement Pipeline view with instrument cards and metric strip"
```

---

## Task 12: Frontend — Pipeline Event Stream Panel

**Files:**
- Create: `web/src/pages/pipeline/EventStream.tsx`
- Modify: `web/src/pages/PipelinePage.tsx`

- [ ] **Step 1: Create EventStream component**

`web/src/pages/pipeline/EventStream.tsx`:
```tsx
import { useEffect, useRef, useState } from 'react';
import styled from 'styled-components';
import { color, border, space, font } from '@/theme';
import type { DebugEvent } from '@/api/types';

interface Props {
  events: DebugEvent[];
}

const statusColor: Record<string, string> = {
  kline_ingested: color.tealAccent,
  provider_fallback: color.yellowPrimary,
  normalization_result: color.tealAccent,
  task_status_changed: color.bluePrimary,
  attempt_completed: color.bluePrimary,
  open_bar_update: color.bluePrimary,
};

export default function EventStream({ events }: Props) {
  const [collapsed, setCollapsed] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [events.length]);

  const recent = events.slice(-50);

  return (
    <Wrap>
      <Header onClick={() => setCollapsed(!collapsed)}>
        <span>Live Event Stream</span>
        <Toggle>{collapsed ? '▸ expand' : '▾ collapse'}</Toggle>
      </Header>
      {!collapsed && (
        <Body ref={scrollRef}>
          {recent.length === 0 && <Empty>Waiting for events...</Empty>}
          {recent.map((ev, i) => {
            const record = ev as Record<string, unknown>;
            const evColor = record.success === false ? color.redAccent : (statusColor[ev.type] ?? color.textGray);
            return (
              <Row key={i}>
                <Dot style={{ background: evColor }} />
                <Type>{ev.type}</Type>
                <Detail>
                  {record.instrument_id && String(record.instrument_id).slice(0, 8)}
                  {record.timeframe && ` · ${record.timeframe}`}
                  {record.provider && ` → ${record.provider}`}
                  {record.latency_ms != null && ` ${record.latency_ms}ms`}
                  {record.error && <ErrorText> {String(record.error)}</ErrorText>}
                </Detail>
              </Row>
            );
          })}
        </Body>
      )}
    </Wrap>
  );
}

const Wrap = styled.div`
  background: ${color.bgWhite};
  border: ${border.std};
  margin-top: ${space.px16}px;
  font-family: ${font.mono};
`;

const Header = styled.div`
  padding: ${space.px10}px ${space.px16}px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  cursor: pointer;
  font-weight: 700;
  font-size: 12px;
  border-bottom: 2px dashed ${color.bgLightGray};
`;

const Toggle = styled.span`
  font-size: 11px;
  color: ${color.textGray};
  font-weight: 400;
`;

const Body = styled.div`
  max-height: 180px;
  overflow-y: auto;
  padding: ${space.px12}px ${space.px16}px;
`;

const Empty = styled.div`
  font-size: 11px;
  color: ${color.textLightGray};
`;

const Row = styled.div`
  display: flex;
  align-items: center;
  gap: ${space.px8}px;
  padding: 3px 0;
  border-bottom: 1px dashed ${color.bgLightGray};
  font-size: 11px;
`;

const Dot = styled.span`
  width: 8px;
  height: 8px;
  flex-shrink: 0;
`;

const Type = styled.span`
  color: ${color.textGray};
  min-width: 140px;
`;

const Detail = styled.span`
  color: ${color.textDark};
`;

const ErrorText = styled.span`
  color: ${color.redAccent};
`;
```

- [ ] **Step 2: Add EventStream to PipelinePage**

In `web/src/pages/PipelinePage.tsx`, import and add at the bottom of the return:
```tsx
import EventStream from './pipeline/EventStream';
// ... at the end of the JSX return, before closing </div>:
<EventStream events={events} />
```

- [ ] **Step 3: Verify in browser**

Run backend + frontend. Events from WebSocket should appear in the live event stream panel at the bottom.

- [ ] **Step 4: Commit**

```bash
git add web/src/pages/pipeline/EventStream.tsx web/src/pages/PipelinePage.tsx
git commit -m "feat(web): add live event stream panel to Pipeline view"
```

---

## Self-Review Checklist

**Spec coverage:**
- [x] Backend DebugEvent type → Task 1
- [x] WebSocket endpoint → Task 2
- [x] Broadcast channel wiring → Task 3
- [x] Event emission from MarketGateway → Task 4
- [x] Frontend scaffold → Task 5
- [x] Design system tokens → Task 6
- [x] AppShell + Sidebar → Task 7
- [x] API client + types → Task 8
- [x] MetricCard → Task 9
- [x] WebSocket client + reconnect → Task 10
- [x] Pipeline page (stats + cards + event stream) → Tasks 11-12
- [ ] Pipeline expanded card details (session bucket progress, provider info) — **deferred to iteration after first vertical slice works**
- [ ] Event emission from orchestrator worker → **deferred to Plan 3 (LLM Trace)**
- [ ] K-Line Charts view → **Plan 2**
- [ ] LLM Trace view → **Plan 3**
- [ ] Cross-view navigation → **Plan 3**

**Placeholder scan:** No TBD/TODO except one annotated `// TODO: wire real WS status` which is resolved in Task 10.

**Type consistency:** `DebugEvent` used consistently across Rust (pa-core) and TypeScript (api/types.ts). `AppState` field name `debug_tx` consistent. MetricCard `accent` prop type matches `MetricAccent` union.
