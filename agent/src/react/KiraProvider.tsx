import React, { createContext, useContext, useMemo } from "react";
import { CoreClientConfig, CoreClient, createKiraClient } from "../core-client/client";

interface KiraContextValue {
  client: CoreClient;
}

const KiraContext = createContext<KiraContextValue | undefined>(undefined);

export interface KiraProviderProps {
  config?: Partial<CoreClientConfig>;
  children: React.ReactNode;
}

export function KiraProvider({ config, children }: KiraProviderProps) {
  const client = useMemo(() => createKiraClient(config), [config]);
  return React.createElement(KiraContext.Provider, { value: { client } }, children);
}

export function useKiraClient(): CoreClient {
  const ctx = useContext(KiraContext);
  if (!ctx) {
    throw new Error(
      "useKiraClient must be used within a <KiraProvider>. " +
        "Wrap your application with <KiraProvider> near the root.",
    );
  }
  return ctx.client;
}
