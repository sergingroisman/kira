#!/usr/bin/env bun
/**
 * Smoke test for CoreClient (no TUI). Validates HTTP+SSE + Agents end-to-end.
 *
 * Run while `kira-cored` is up:
 *   bun run src/cli/smoke.ts
 */

import { CoreClient } from "../core-client/client.js";

const client = new CoreClient();

console.log("=== health ===");
const h = await client.health();
console.log(h);

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

console.log("\n=== spawn writer + poll ===");
const agent = await client.spawnAgent({
  kind: "writer",
  task: "Crie src/content/test.md com 1 frase sobre Bun runtime. Use file_write e depois done.",
  worktree_id: null,
  base: null,
  budget_usd: null,
  max_iters: 5,
});
console.log("spawned:", agent.id, "wt:", agent.worktree_id);

let last = agent;
for (let i = 0; i < 30; i++) {
  await new Promise((r) => setTimeout(r, 2000));
  last = await client.getAgent(agent.id);
  console.log(
    `  [${i}] status=${last.status} iters=${last.iters} usd=${last.usd_spent.toFixed(4)} model=${last.model ?? "-"}`,
  );
  if (["completed", "failed", "cancelled", "stopped"].includes(last.status)) break;
}

console.log("\n=== final ===");
console.log({
  status: last.status,
  iters: last.iters,
  usd: last.usd_spent,
  result: last.result?.slice(0, 200),
  error: last.error,
});

console.log("\nall good ✓");
