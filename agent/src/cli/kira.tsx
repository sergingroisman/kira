#!/usr/bin/env bun
/**
 * Kira REPL entrypoint (Bun runtime).
 *
 * Talks to kira-cored at http://127.0.0.1:7777 by default.
 * Override via env KIRA_CORE_URL.
 *
 * Usage:
 *   bun run src/cli/kira.tsx
 *   KIRA_CORE_URL=http://other:7777 bun run src/cli/kira.tsx
 */

import React from "react";
import { render } from "ink";
import { KiraApp } from "../ui/KiraApp.js";
import { CoreClient } from "../core-client/client.js";

const client = new CoreClient();

const { waitUntilExit } = render(<KiraApp client={client} />, {
  exitOnCtrlC: true,
});

await waitUntilExit();
