import { useDebugEventStore } from './debugEventStore';
import type { DebugEvent } from '@/api/types';

let ws: WebSocket | null = null;
let retryCount = 0;
let retryTimer: ReturnType<typeof setTimeout> | null = null;
let manualClose = false;

function wsUrl(): string {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  return `${proto}://${location.host}/api/ws`;
}

function scheduleReconnect() {
  if (manualClose) return;
  const delay = Math.min(1000 * Math.pow(2, retryCount), 30_000);
  retryTimer = setTimeout(() => {
    retryCount += 1;
    connectWs();
  }, delay);
}

export function connectWs(): void {
  if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
    return;
  }

  manualClose = false;
  const { setConnected, push } = useDebugEventStore.getState();

  ws = new WebSocket(wsUrl());

  ws.onopen = () => {
    retryCount = 0;
    setConnected(true);
  };

  ws.onmessage = (ev: MessageEvent) => {
    try {
      const event: DebugEvent = JSON.parse(ev.data as string);
      push(event);
    } catch {
      // ignore malformed messages
    }
  };

  ws.onclose = () => {
    setConnected(false);
    ws = null;
    scheduleReconnect();
  };

  ws.onerror = () => {
    // onclose will fire after onerror, which triggers reconnect
  };
}

export function disconnectWs(): void {
  manualClose = true;
  if (retryTimer !== null) {
    clearTimeout(retryTimer);
    retryTimer = null;
  }
  if (ws) {
    ws.close();
    ws = null;
  }
  useDebugEventStore.getState().setConnected(false);
}
