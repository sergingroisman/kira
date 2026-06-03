/**
 * MessageBubble — styled message display for user and assistant messages.
 *
 * Muted borders, reduced visual weight.
 */
import React from "react";
import { Box, Text } from "../../ink/kira-ink.js";
import { md2ansi } from "../markdown.js";

export interface MessageBubbleProps {
  role: "user" | "assistant";
  content: string;
  metadata?: string;
}

export const MessageBubble: React.FC<MessageBubbleProps> = ({
  role,
  content,
  metadata,
}) => {
  if (role === "user") {
    return (
      <Box flexDirection="column" marginBottom={1}>
        <Box paddingLeft={1} borderLeft={"┃"} borderLeftColor="#2E5C3A">
          <Text color="#2E7D32" dimColor>
            {" "}
            You
          </Text>
        </Box>
        <Box paddingLeft={2}>
          <Text color="#C8C8C8">{content}</Text>
        </Box>
      </Box>
    );
  }

  // Assistant
  return (
    <Box flexDirection="column" marginBottom={1}>
      <Box paddingLeft={1} borderLeft={"┃"} borderLeftColor="#3A5068">
        <Text color="#555555" dimColor>
          {" "}
          Kira
        </Text>
      </Box>
      <Box paddingLeft={2}>
        <Text color="#C8C8C8">{md2ansi(content)}</Text>
      </Box>
      {metadata && (
        <Box paddingLeft={2}>
          <Text color="#555555">{metadata}</Text>
        </Box>
      )}
    </Box>
  );
};

export default MessageBubble;
