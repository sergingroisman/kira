import React, { createContext, useContext, useState, useCallback, useEffect, useRef } from "react";
import { Box, Text } from "../../ink/kira-ink.js";

export type ToastType = "info" | "success" | "warning" | "error";

export interface ToastItem {
  id: string;
  type: ToastType;
  message: string;
  createdAt: number;
}

interface ToastCtx {
  toasts: ToastItem[];
  addToast: (type: ToastType, message: string) => void;
}

const ToastContext = createContext<ToastCtx>({ toasts: [], addToast: () => {} });

const TOAST_DURATION = 3000;
const MAX_TOASTS = 5;

const TOAST_COLORS: Record<ToastType, string> = {
  info: "#87CEEB",
  success: "#4ADE80",
  warning: "#FBBF24",
  error: "#EF4444",
};

let nextId = 1;

export const ToastProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const addToast = useCallback((type: ToastType, message: string) => {
    const id = String(nextId++);
    setToasts((prev) => {
      const next = [...prev, { id, type, message, createdAt: Date.now() }];
      return next.slice(-MAX_TOASTS);
    });
  }, []);

  return (
    <ToastContext.Provider value={{ toasts, addToast }}>
      {children}
    </ToastContext.Provider>
  );
};

export function useToast(): ToastCtx {
  return useContext(ToastContext);
}

export const ToastContainer: React.FC<{ toasts?: ToastItem[] }> = ({ toasts: propToasts }) => {
  const { toasts: ctxToasts } = useToast();
  const toasts = propToasts ?? ctxToasts;
  const [, forceUpdate] = useState(0);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    timerRef.current = setInterval(() => forceUpdate((n) => n + 1), 200);
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, []);

  const alive = toasts.filter((t) => Date.now() - t.createdAt < TOAST_DURATION);

  if (alive.length === 0) return null;

  return (
    <Box flexDirection="column" marginBottom={1}>
      {alive.map((t) => {
        const elapsed = Date.now() - t.createdAt;
        const fade = Math.max(0, 1 - elapsed / TOAST_DURATION);
        const color = TOAST_COLORS[t.type];

        return (
          <Box key={t.id} flexDirection="row">
            <Text color={color} dimColor={fade < 0.3}>{"\u2502 "}</Text>
            <Text color={color} dimColor={fade < 0.3}>
              {t.message}
            </Text>
          </Box>
        );
      })}
    </Box>
  );
};
