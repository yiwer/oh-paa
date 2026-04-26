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
