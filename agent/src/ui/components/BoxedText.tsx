/**
 * BoxedText — text inside a unicode box with border.
 */
import React from "react";
import { Box, Text } from "../../ink/kira-ink.js";

export interface BoxedTextProps {
  children: React.ReactNode;
  borderColor?: string;
  padding?: number;
  title?: string;
}

export const BoxedText: React.FC<BoxedTextProps> = ({
  children,
  borderColor = "gray",
  padding = 1,
  title,
}) => {
  const content = React.Children.toArray(children);
  const lines = content.map((c) => {
    if (typeof c === "string") return c;
    if (typeof c === "number") return String(c);
    if (React.isValidElement(c) && "props" in c) {
      // For Text elements, just extract children
      const child = (c as React.ReactElement).props.children;
      if (typeof child === "string") return child;
      return String(child ?? "");
    }
    return String(c);
  });

  const maxLen = Math.max(...lines.map((l) => l.length), title?.length ?? 0) + padding * 2;
  const topBorder = "┌" + "─".repeat(maxLen) + "┐";
  const bottomBorder = "└" + "─".repeat(maxLen) + "┘";

  const contentLines = lines.map((line) => {
    const pad = " ".repeat(padding);
    const text = `${pad}${line}${" ".repeat(maxLen - line.length - padding)}`;
    return `│${text}│`;
  });

  const titleLine = title
    ? `│ ${title}${" ".repeat(maxLen - title.length - 2)} │`
    : null;

  return (
    <Box flexDirection="column">
      <Text color={borderColor}>{topBorder}</Text>
      {titleLine && <Text color={borderColor}>{titleLine}</Text>}
      {contentLines.map((line, i) => (
        <Text color={borderColor} key={i}>
          {line}
        </Text>
      ))}
      <Text color={borderColor}>{bottomBorder}</Text>
    </Box>
  );
};

export default BoxedText;
