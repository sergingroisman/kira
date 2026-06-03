import { useContext } from 'react';
import { TerminalSizeContext } from '../components/TerminalSizeContext.js';

export function useTerminalSize(): { columns: number; rows: number } {
  const ctx = useContext(TerminalSizeContext);
  return {
    columns: ctx?.columns ?? 80,
    rows: ctx?.rows ?? 24,
  };
}
