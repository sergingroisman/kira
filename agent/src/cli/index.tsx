import "dotenv/config";
import React from "react";
import { render } from "ink";
import { App } from "../ui/App.js";

// Validate required env vars before rendering
const missingVars: string[] = [];
if (!process.env.DEEPSEEK_API_KEY) missingVars.push("DEEPSEEK_API_KEY");

if (missingVars.length > 0) {
  console.error(`Missing required environment variables: ${missingVars.join(", ")}`);
  console.error("Copy agent/.env.example to agent/.env and fill in values.");
  process.exit(1);
}

// System prompt guides DeepSeek's tool selection and response style.
const SYSTEM_PROMPT = `You are Sergin, an autonomous agent with access to the following tools:
- Bash: Execute shell commands
- FileRead: Read file contents
- FileWrite: Write content to files
- FileAppend: Append content to files
- WebSearch: Search the web via Tavily

Use tools proactively when tasks require it. After tool results, incorporate them into your response. Always respond in the same language as the user's question.`;

const { waitUntilExit } = render(<App systemPrompt={SYSTEM_PROMPT} />);
await waitUntilExit();
