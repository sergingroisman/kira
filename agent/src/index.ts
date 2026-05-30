// Re-exports for ergonomic imports from 'kira-agent'.
export { CoreClient, CoreError, CoreClientConfig, DEFAULT_CONFIG, McpServersResponse } from './core-client/client.ts';
export type {
  ChatRequest,
  ChatMessage,
  ChatResponse,
  StreamEvent,
  YoloResponse,
  BashRequest,
  BashResponse,
  FileReadRequest,
  FileReadResponse,
  FileWriteRequest,
  FileWriteResponse,
  Agent,
  AgentKind,
  ApprovalRequest,
  ApprovalResponse,
  CancelResponse,
  PendingApproval,
  SpawnAgentRequest,
  Plan,
  DraftRequest,
  ApproveResponse,
  CancelPlanResponse,
  Lesson,
  CreateLessonRequest,
  DeleteLessonResponse,
  PromoteResponse,
  RunGatesRequest,
  RunGatesResponse,
  RunEvalRequest,
  RunEvalResponse,
  RunReviewRequest,
  RunReviewResponse,
  StatsResponse,
  ModelUsage,
  SddRun,
  StartSddRequest,
  SddListResponse,
  PlanQuestion,
  PlanOption,
  AnswerQuestionsRequest,
  PillarEvent,
  Worktree,
  Claim,
  WriteDecision,
} from './core-client/client.ts';

/** Convenience factory that creates a CoreClient with default configuration. */
export function createKiraClient(overrides?: Partial<CoreClientConfig>): CoreClient {
  const config = { ...DEFAULT_CONFIG, ...overrides };
  return new CoreClient(config);
}

// Also re-export as default for `import Kira from 'kira-agent'`.
export { default } from './core-client/client.ts';

// React integration
export { KiraProvider, useKiraClient, useKira } from './react';
export type { KiraProviderProps } from './react';
