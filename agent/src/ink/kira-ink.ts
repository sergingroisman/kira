import { type ReactNode } from 'react';
import { default as _Box } from './components/Box.js';
import type { Props as _BoxProps } from './components/Box.js';
import { default as _Text } from './components/Text.js';
import type { Props as _TextProps } from './components/Text.js';
import { AlternateScreen as _AlternateScreen } from './components/AlternateScreen.js';

export { default as render, createRoot, renderSync } from './root.js';
export { default as Ink } from './ink.js';

export { default as ScrollBox } from './components/ScrollBox.js';
export { default as Spacer } from './components/Spacer.js';
export { default as Newline } from './components/Newline.js';

export { default as useInput } from './hooks/use-input.js';
export { default as useStdin } from './hooks/use-stdin.js';
export { default as useApp } from './hooks/use-app.js';
export { useTerminalViewport } from './hooks/use-terminal-viewport.js';
export { useTerminalSize } from './hooks/use-terminal-size.js';

export function useStdout(): { stdout: NodeJS.WriteStream } {
  return { stdout: process.stdout };
}

type KiraTextProps = _TextProps & { dimColor?: boolean };
type KiraBoxProps = _BoxProps & { children?: ReactNode };

export const Box = _Box as unknown as (props: KiraBoxProps) => ReactNode;
export const Text = _Text as unknown as (props: KiraTextProps) => ReactNode;
export const AlternateScreen = _AlternateScreen as unknown as (props: { mouseTracking?: boolean; children?: ReactNode }) => ReactNode;

export type BoxProps = _BoxProps;
export type TextProps = _TextProps;
