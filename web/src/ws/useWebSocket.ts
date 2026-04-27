import { useEffect } from 'react';
import { connectWs, disconnectWs } from './client';

export function useWebSocket() {
  useEffect(() => {
    connectWs();
    return () => disconnectWs();
  }, []);
}
