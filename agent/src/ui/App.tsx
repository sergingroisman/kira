import React, { useState, useCallback, useRef } from "react";
import { Box, Text, Static } from "ink";
import { HumanMessage, SystemMessage, AIMessage } from "@langchain/core/messages";
import type { BaseMessage } from "@langchain/core/messages";
import { graph, makeSessionConfig } from "../graph/graph.js";
import { InputBar } from "./InputBar.js";
import { StreamingOutput } from "./StreamingOutput.js";
import { Spinner } from "./Spinner.js";
import { ToolCallPanel } from "./ToolCallPanel.js";

interface Message {
  role: "user" | "assistant";
  content: string;
}

interface ToolCallEntry {
  name: string;
  args: string;
  status: "running" | "done" | "error";
  result?: string;
}

interface AppProps {
  sessionId?: string;
  systemPrompt?: string;
}

export function App({ sessionId, systemPrompt }: AppProps) {
  const [history, setHistory] = useState<Message[]>([]);
  const [streaming, setStreaming] = useState("");
  const [isThinking, setIsThinking] = useState(false);
  const [toolCalls, setToolCalls] = useState<ToolCallEntry[]>([]);

  // Accumulate ALL messages across turns (workaround for MemorySaver not
  // persisting ToolMessages in streamMode: "messages" — LangGraph 1.3.0 limitation)
  const messageHistory = useRef<BaseMessage[]>([]);

  // Session config is stable for the lifetime of the process (one thread_id per session — D-06)
  const sessionConfig = useRef(makeSessionConfig());

  // Track whether system prompt has been sent (sent once per session lifetime)
  const systemPromptSent = useRef(false);

  // Token accumulator ref — avoids excessive setState on rapid token arrival (Pitfall 4)
  const tokenBuffer = useRef("");
  const flushTimer = useRef<ReturnType<typeof setInterval> | null>(null);

  const startFlush = useCallback(() => {
    if (flushTimer.current) return;
    flushTimer.current = setInterval(() => {
      if (tokenBuffer.current) {
        setStreaming(prev => prev + tokenBuffer.current);
        tokenBuffer.current = "";
      }
    }, 16); // ~60fps flush rate
  }, []);

  const stopFlush = useCallback(() => {
    if (flushTimer.current) {
      clearInterval(flushTimer.current);
      flushTimer.current = null;
    }
    // Flush any remaining tokens
    if (tokenBuffer.current) {
      setStreaming(prev => prev + tokenBuffer.current);
      tokenBuffer.current = "";
    }
  }, []);

  const handleSubmit = useCallback(async (userInput: string) => {
    setIsThinking(true);
    setStreaming("");
    setToolCalls([]);
    tokenBuffer.current = "";

    // Add user message to visible history immediately
    setHistory(prev => [...prev, { role: "user", content: userInput }]);

    startFlush();

    let fullResponse = "";

    try {
      // Build COMPLETE message list for this turn (workaround for MemorySaver
      // not persisting ToolMessages in streamMode: "messages"):
      //   1. system prompt (first turn only)
      //   2. all prior messages
      //   3. new user input
      const messages: BaseMessage[] = [];
      if (!systemPromptSent.current && systemPrompt) {
        messages.push(new SystemMessage(systemPrompt));
        systemPromptSent.current = true;
      }
      // Include accumulated message history for full context
      for (const msg of messageHistory.current) {
        messages.push(msg);
      }
      messages.push(new HumanMessage(userInput));

      // Use a fresh thread per turn to avoid MemorySaver ToolMessage corruption
      // (LangGraph 1.3.0 known issue with streamMode: "messages" + tool calls)
      const turnConfig = {
        configurable: { thread_id: crypto.randomUUID() },
        streamMode: "messages" as const,
      };

      // Track tool calls seen during this turn
      const seenToolCallNames = new Set<string>();

      for await (const [chunk] of await graph.stream(
        { messages },
        turnConfig
      )) {
        // Only capture text from AI messages — skip ToolMessage content
        // (e.g. search result JSON from executeTool node). LangGraph yields
        // chunks from ALL nodes in streamMode: "messages".
        const isAi = (chunk as any)._getType?.() === "ai";
        if (isAi) {
          const text = typeof chunk.content === "string" ? chunk.content : "";
          if (text) {
            tokenBuffer.current += text;
            fullResponse += text;
          }
        }

        // Track tool calls from LLM response chunks
        // LangGraph sets tool_calls at the top level on AIMessage chunks;
        // streamed chunks may use tool_call_chunks with partial args
        const chunkToolCalls = (chunk as any).tool_calls
          ?? (chunk as any).additional_kwargs?.tool_calls
          ?? (chunk as any).tool_call_chunks;
        if (chunkToolCalls) {
          for (const tc of chunkToolCalls) {
            const name = tc.name ?? tc.function?.name;
            const rawArgs = tc.args ?? (tc.function?.arguments ? JSON.parse(tc.function.arguments) : undefined);
            const args = rawArgs ? JSON.stringify(rawArgs) : "";
            if (name && !seenToolCallNames.has(name)) {
              seenToolCallNames.add(name);
              setToolCalls(prev => [...prev, {
                name,
                args,
                status: "running" as const,
              }]);
            } else if (name && args && args !== "{}") {
              // Update args from later chunks that may have fuller data
              setToolCalls(prev => prev.map(tc =>
                tc.name === name ? { ...tc, args } : tc
              ));
            }
          }
        }
      }
      // After stream completes, mark all tool calls as done
      setToolCalls(prev => prev.map(tc => ({ ...tc, status: "done" as const })));

      // Persist the assistant response for the next turn's full message list
      messageHistory.current.push(new HumanMessage(userInput));
      messageHistory.current.push(
        new AIMessage({ content: fullResponse })
      );
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      fullResponse = `Error: ${message}`;
      tokenBuffer.current = fullResponse;
      // Mark any running tool calls as errored
      setToolCalls(prev => prev.map(tc => ({ ...tc, status: "error" as const })));
    } finally {
      stopFlush();
      setIsThinking(false);
      setHistory(prev => [...prev, { role: "assistant", content: fullResponse }]);
      setStreaming("");
    }
  }, [startFlush, stopFlush, systemPrompt]);

  return (
    <Box flexDirection="column" paddingX={1}>
      {/* Static renders completed history without re-rendering previous items */}
      <Static items={history}>
        {(msg, i) => (
          <Box key={i} flexDirection="column" marginBottom={1}>
            <Text color={msg.role === "user" ? "green" : "white"} bold={msg.role === "user"}>
              {msg.role === "user" ? "You" : "Sergin"}
            </Text>
            <Text>{msg.content}</Text>
          </Box>
        )}
      </Static>

      {isThinking && <Spinner />}
      {toolCalls.length > 0 && (
        <Box flexDirection="column" marginY={1}>
          {toolCalls.map((tc, i) => (
            <ToolCallPanel
              key={i}
              toolName={tc.name}
              args={tc.args}
              result={tc.result}
              status={tc.status}
            />
          ))}
        </Box>
      )}
      {streaming && <StreamingOutput tokens={streaming} />}

      <InputBar onSubmit={handleSubmit} disabled={isThinking} />
    </Box>
  );
}
