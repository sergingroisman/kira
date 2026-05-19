import { HumanMessage, AIMessage } from "@langchain/core/messages";
import { graph } from "../src/graph/graph.js";

const messageHistory = [];

// Turn 1: simple chat
const stream1 = await graph.stream(
  { messages: [...messageHistory, new HumanMessage("Minha cor preferida é azul")] },
  { configurable: { thread_id: crypto.randomUUID() }, streamMode: "messages" }
);
let r1 = "";
for await (const [chunk] of stream1) {
  const t = typeof chunk.content === "string" ? chunk.content : "";
  if (t) r1 += t;
}
messageHistory.push(new HumanMessage("Minha cor preferida é azul"), new AIMessage({ content: r1 }));
console.log("Turn 1 OK:", r1.substring(0, 80));

// Turn 2: check memory
const stream2 = await graph.stream(
  { messages: [...messageHistory, new HumanMessage("Qual a minha cor preferida?")] },
  { configurable: { thread_id: crypto.randomUUID() }, streamMode: "messages" }
);
let r2 = "";
for await (const [chunk] of stream2) {
  const t = typeof chunk.content === "string" ? chunk.content : "";
  if (t) r2 += t;
}
messageHistory.push(new HumanMessage("Qual a minha cor preferida?"), new AIMessage({ content: r2 }));
console.log("Turn 2 OK:", r2.substring(0, 80));

// Turn 3: ask for web search
const stream3 = await graph.stream(
  { messages: [...messageHistory, new HumanMessage("Pesquise preço de passagens Recife Portugal setembro 2026")] },
  { configurable: { thread_id: crypto.randomUUID() }, streamMode: "messages" }
);
let r3 = "";
try {
  for await (const [chunk] of stream3) {
    const t = typeof chunk.content === "string" ? chunk.content : "";
    if (t) r3 += t;
  }
  messageHistory.push(new HumanMessage("Pesquise preço de passagens Recife Portugal setembro 2026"), new AIMessage({ content: r3 }));
  console.log("Turn 3 OK:", r3.substring(0, 150));
} catch(e) {
  console.error("Turn 3 ERROR:", String(e).substring(0, 300));
  process.exit(1);
}

// Turn 4: what happened? (THIS IS WHERE THE BUG WAS)
const stream4 = await graph.stream(
  { messages: [...messageHistory, new HumanMessage("cadê?")] },
  { configurable: { thread_id: crypto.randomUUID() }, streamMode: "messages" }
);
let r4 = "";
try {
  for await (const [chunk] of stream4) {
    const t = typeof chunk.content === "string" ? chunk.content : "";
    if (t) r4 += t;
  }
  console.log("Turn 4 OK! No tool call corruption:", r4.substring(0, 200));
} catch(e) {
  console.error("Turn 4 ERROR (bug still present):", String(e).substring(0, 500));
  process.exit(1);
}

console.log("\n✓ Bug fixed! Full message history approach works.");
