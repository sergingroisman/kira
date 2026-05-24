#!/usr/bin/env node
// Thin entry point: delegates to TypeScript source via tsx.
// tsx handles ESM + TypeScript transparently; no compilation step needed for local dev.
//
// Points to the core daemon REPL (kira.tsx), NOT the legacy standalone path.
import { spawnSync } from "child_process";
import { fileURLToPath } from "url";
import { dirname, resolve } from "path";

const __dirname = dirname(fileURLToPath(import.meta.url));
// tsx is hoisted to the monorepo root by npm workspaces
const tsxBin = resolve(__dirname, "../../node_modules/.bin/tsx");
const entrypoint = resolve(__dirname, "../src/cli/kira.tsx");

// Load .env file if it exists alongside the bin
const envFile = resolve(__dirname, "../.env");

const result = spawnSync(
  process.execPath,
  [tsxBin, "--env-file", envFile, entrypoint, ...process.argv.slice(2)],
  { stdio: "inherit" }
);

process.exit(result.status ?? 0);
