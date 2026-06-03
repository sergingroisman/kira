import React, { useState, useCallback } from "react";
import { Box, Text, useInput } from "../../ink/kira-ink.js";
import type { PlanQuestion } from "../../generated/sdd.js";

export interface PlanQuestionsProps {
  sddRunId: string;
  questions: PlanQuestion[];
  onSubmit: (answers: Record<string, string[]>) => void;
  onCancel?: () => void;
}

export const PlanQuestions: React.FC<PlanQuestionsProps> = ({
  questions,
  onSubmit,
  onCancel,
}) => {
  const [qIdx, setQIdx] = useState(0);
  const [optIdx, setOptIdx] = useState(0);
  // answers: question_id → Set of selected labels
  const [answers, setAnswers] = useState<Record<string, Set<string>>>({});

  const current = questions[qIdx];

  const toggleOption = useCallback(
    (label: string) => {
      if (!current) return;
      setAnswers((prev) => {
        const set = new Set(prev[current.id] ?? []);
        if (current.multi_select) {
          if (set.has(label)) set.delete(label);
          else set.add(label);
        } else {
          set.clear();
          set.add(label);
        }
        return { ...prev, [current.id]: set };
      });
    },
    [current],
  );

  const canAdvance = current
    ? (answers[current.id]?.size ?? 0) > 0
    : false;

  useInput(
    (input, key) => {
      if (!current) return;

      if (key.upArrow) {
        setOptIdx((i) => Math.max(0, i - 1));
        return;
      }
      if (key.downArrow) {
        setOptIdx((i) => Math.min(current.options.length - 1, i + 1));
        return;
      }

      if (input === " " || key.return) {
        const label = current.options[optIdx]?.label;
        if (label) {
          toggleOption(label);
          // Single-select: auto-advance after selection
          if (!current.multi_select && key.return) {
            advance();
          }
        }
        return;
      }

      if (key.escape) {
        onCancel?.();
        return;
      }

      // Enter on multi-select: advance
      if (key.return && current.multi_select && canAdvance) {
        advance();
        return;
      }
    },
    { isActive: true },
  );

  const advance = () => {
    if (!canAdvance) return;
    if (qIdx < questions.length - 1) {
      setQIdx((i) => i + 1);
      setOptIdx(0);
    } else {
      // All questions answered — build final answer map
      const result: Record<string, string[]> = {};
      for (const q of questions) {
        result[q.id] = Array.from(answers[q.id] ?? []);
      }
      onSubmit(result);
    }
  };

  if (!current) return null;

  const selectedLabels = answers[current.id] ?? new Set();

  return (
    <Box
      flexDirection="column"
      borderStyle="round"
      borderColor="cyan"
      paddingX={1}
      marginTop={1}
    >
      <Box marginBottom={1} justifyContent="space-between">
        <Text color="cyan" bold>
          {" "}SPECIFY — Clarifying Questions{" "}
        </Text>
        <Text dimColor>
          {qIdx + 1}/{questions.length}
        </Text>
      </Box>

      <Box marginBottom={1}>
        <Text bold>{current.question}</Text>
      </Box>

      {current.multi_select && (
        <Box marginBottom={1}>
          <Text dimColor>(Space to select multiple, Enter to confirm)</Text>
        </Box>
      )}

      <Box flexDirection="column">
        {current.options.map((opt, i) => {
          const focused = i === optIdx;
          const selected = selectedLabels.has(opt.label);
          const indicator = current.multi_select
            ? selected
              ? "◉"
              : "○"
            : selected
              ? "●"
              : "○";

          return (
            <Box key={opt.label} flexDirection="column" marginBottom={0}>
              <Box flexDirection="row" gap={1}>
                <Text color={focused ? "cyan" : selected ? "green" : "white"} bold={focused}>
                  {focused ? "▸" : " "} {indicator}
                </Text>
                <Text color={focused ? "cyan" : selected ? "green" : "white"} bold={focused || selected}>
                  {opt.label}
                </Text>
              </Box>
              {focused && opt.description && (
                <Box paddingLeft={5}>
                  <Text dimColor>{opt.description}</Text>
                </Box>
              )}
            </Box>
          );
        })}
      </Box>

      <Box marginTop={1} borderStyle="single" borderColor="gray" paddingX={1}>
        <Text dimColor>
          ↑↓ navigate  {current.multi_select ? "Space select  Enter confirm" : "Enter select"}  Esc cancel
        </Text>
      </Box>
    </Box>
  );
};
