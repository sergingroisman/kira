#!/usr/bin/env bun
/**
 * Smoke test for CoreClient (no TUI). Validates HTTP+SSE end-to-end.
 *
 * Run while `kira-cored` is up:
 *   bun run src/cli/smoke.ts
 */

import { CoreClient } from "../core-client/client.js";

const client = new CoreClient();

console.log("=== health ===");
const h = await client.health();
console.log(h);

console.log("\n=== non-streaming chat (standard tier) ===");
const r = await client.chat({
  tier: "standard",
  model: null,
  messages: [{ role: "user", content: "diga ola em 3 palavras" }],
});
console.log(r);

console.log("\n=== streaming chat (light tier) ===");
process.stdout.write("delta: ");
for await (const ev of client.chatStream({
  tier: "light",
  model: null,
  messages: [{ role: "user", content: "conte 1 2 3" }],
})) {
  if (ev.type === "token") process.stdout.write(ev.delta);
  if (ev.type === "done") {
    console.log(`\n[done model=${ev.model} reason=${ev.finish_reason}]`);
  }
  if (ev.type === "error") console.error("ERROR:", ev.message);
}

console.log("\n=== bash tool (echo) ===");
const b = await client.bash({ argv: ["echo", "kira-smoke-ok"], cwd: null, timeout_secs: 5 });
console.log(b);

console.log("\nall good ✓");
