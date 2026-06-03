import React, { useState, useCallback } from "react";
import { Box, Text, useInput } from "../../ink/kira-ink.js";
import type { PendingApproval } from "../../generated/agent.js";

export interface ApprovalPromptProps {
  agentId: string;
  agentKind: string;
  approval: PendingApproval;
  onDecide: (approved: boolean, feedback?: string) => void;
}

type Stage = "confirm" | "feedback";

export const ApprovalPrompt: React.FC<ApprovalPromptProps> = ({
  agentId,
  agentKind,
  approval,
  onDecide,
}) => {
  const [stage, setStage] = useState<Stage>("confirm");
  const [feedback, setFeedback] = useState("");

  const handleInput = useCallback(
    (input: string, key: { return: boolean; escape: boolean; backspace: boolean; delete: boolean }) => {
      if (stage === "confirm") {
        if (input === "y" || input === "Y") {
          onDecide(true);
          return;
        }
        if (input === "n" || input === "N") {
          setStage("feedback");
          return;
        }
        if (key.escape) {
          // Escape = reject without feedback
          onDecide(false);
          return;
        }
      } else {
        // feedback stage
        if (key.return) {
          onDecide(false, feedback.trim() || undefined);
          return;
        }
        if (key.escape) {
          onDecide(false);
          return;
        }
        if (key.backspace || key.delete) {
          setFeedback((f) => f.slice(0, -1));
          return;
        }
        if (input && !key.return) {
          setFeedback((f) => f + input);
        }
      }
    },
    [stage, feedback, onDecide],
  );

  useInput(handleInput);

  // Truncate long args for display
  const argsDisplay =
    approval.arguments.length > 200
      ? `${approval.arguments.slice(0, 200)}…`
      : approval.arguments;

  return (
    <Box
      flexDirection="column"
      borderStyle="round"
      borderColor="yellow"
      paddingX={1}
      marginTop={1}
    >
      <Box marginBottom={1}>
        <Text color="yellow" bold>
          {" "}SC2 APPROVAL REQUIRED{" "}
        </Text>
      </Box>

      <Box flexDirection="row" gap={1}>
        <Text dimColor>Agent:</Text>
        <Text color="cyan">{agentKind}</Text>
        <Text dimColor>{agentId.slice(0, 8)}</Text>
      </Box>

      <Box flexDirection="row" gap={1}>
        <Text dimColor>Tool:</Text>
        <Text color="magenta" bold>
          {approval.tool_name}
        </Text>
      </Box>

      <Box marginTop={1} flexDirection="column">
        <Text dimColor>Arguments:</Text>
        <Box paddingLeft={2}>
          <Text color="white">{argsDisplay}</Text>
        </Box>
      </Box>

      <Box marginTop={1} borderStyle="single" borderColor="gray" paddingX={1}>
        {stage === "confirm" ? (
          <Text>
            <Text color="green" bold>
              [Y]
            </Text>
            <Text> Approve  </Text>
            <Text color="red" bold>
              [N]
            </Text>
            <Text> Reject  </Text>
            <Text dimColor>[Esc] Reject</Text>
          </Text>
        ) : (
          <Box flexDirection="column">
            <Text dimColor>Rejection feedback (optional, Enter to confirm):</Text>
            <Box marginTop={0}>
              <Text color="gray">&gt; </Text>
              <Text>{feedback}</Text>
              <Text color="yellow">█</Text>
            </Box>
          </Box>
        )}
      </Box>
    </Box>
  );
};
