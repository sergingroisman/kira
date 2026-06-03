/**
 * Attachment Parser — detects `@path/file` syntax in user input,
 * extracts file references, resolves paths, and reads file content.
 *
 * Inspired by Claude Code's @-mention system.
 *
 * Usage:
 *   const { text, attachments } = parseAttachments(input, cwd);
 *   // text = user message with @paths stripped
 *   // attachments = resolved files ready for injection
 */

import { existsSync, readFileSync, statSync } from "node:fs";
import { resolve, dirname, isAbsolute } from "node:path";
import { homedir } from "node:os";

// -- Types ---------------------------------------------------------------------

export type AttachmentType = "file" | "image";

export interface FileAttachment {
  type: "file";
  path: string;         // relative path for display
  absPath: string;      // absolute path for resolution
  content: string;      // file content
  numLines: number;
  truncated: boolean;
  mimeType: string;
}

export interface ImageAttachment {
  type: "image";
  path: string;         // relative path for display
  absPath: string;      // absolute path for resolution
  data: string;         // base64 encoded
  mediaType: string;    // "image/png", "image/jpeg", etc.
  size: number;         // bytes
}

export type Attachment = FileAttachment | ImageAttachment;

export interface ParseResult {
  text: string;         // user text with @paths stripped
  attachments: Attachment[];
}

// -- Constants -----------------------------------------------------------------

const MAX_TEXT_SIZE = 50_000;     // 50KB limit for text files
const MAX_IMAGE_SIZE = 5_000_000; // 5MB limit for images
const MAX_LINES_TRUNCATE = 500;   // max lines before truncation

const TEXT_EXTENSIONS = new Set([
  ".rs", ".md", ".toml", ".yaml", ".yml", ".json",
  ".ts", ".tsx", ".js", ".jsx", ".py", ".sh",
  ".css", ".html", ".xml", ".sql", ".go", ".c", ".cpp",
  ".h", ".hpp", ".java", ".rb", ".swift", ".kt",
  ".lua", ".zig", ".nim", ".ex", ".exs",
]);

const IMAGE_EXTENSIONS = new Set([
  ".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".bmp", ".ico",
]);

const SUPPORTED_EXTENSIONS = new Set([
  ...Array.from(TEXT_EXTENSIONS), ...Array.from(IMAGE_EXTENSIONS),
]);

// -- Regex (adapted from Claude Mythos) ----------------------------------------

// Detect @token at end of string (for autocomplete triggering)
export const HAS_AT_SYMBOL_RE = /(^|\s)@([\p{L}\p{N}\p{M}_\-./\\()[\]~:]*|"[^"]*"?)$/u;

// Match @token in full string (for extraction)
const AT_MENTION_RE = /(^|\s)@([^\s]+)/g;
const QUOTED_AT_MENTION_RE = /(^|\s)@"([^"]+)"/g;

// Line range: @file.rs#L10-20
const LINE_RANGE_RE = /^(.+?)#L(\d+)(?:-(\d+))?$/;

// -- Path Resolution -----------------------------------------------------------

/**
 * Expand a path that may start with ~/ or be relative.
 * 
 * Examples:
 *   expandPath("~/Cargo.toml", "/workspace") → "/home/user/Cargo.toml"
 *   expandPath("/absolute/path", "/workspace") → "/absolute/path"
 *   expandPath("./src/main.rs", "/workspace") → "/workspace/src/main.rs"
 *   expandPath("Cargo.toml", "/workspace") → "/workspace/Cargo.toml"
 */
export function expandPath(path: string, baseDir: string): string {
  // Strip leading @ if present
  let p = path.startsWith("@") ? path.slice(1) : path;
  
  // Strip line range suffix for path resolution
  const lineRangeMatch = p.match(LINE_RANGE_RE);
  if (lineRangeMatch) {
    p = lineRangeMatch[1];
  }

  // Handle ~/
  if (p.startsWith("~/")) {
    return resolve(homedir(), p.slice(2));
  }

  // Handle absolute paths
  if (isAbsolute(p)) {
    return resolve(p);
  }

  // Handle ./relative or bare relative
  return resolve(baseDir, p);
}

/**
 * Parse line range from @mention, e.g., "@file.rs#L10-20"
 * Returns { path, startLine, endLine } or null if no line range.
 */
export function parseLineRange(mention: string): { path: string; startLine?: number; endLine?: number } | null {
  const match = mention.match(LINE_RANGE_RE);
  if (!match) return null;
  
  return {
    path: match[1],
    startLine: parseInt(match[2], 10),
    endLine: match[3] ? parseInt(match[3], 10) : undefined,
  };
}

// -- Extraction ----------------------------------------------------------------

/**
 * Extract all @-mentioned file paths from user input.
 * Handles both quoted (@"file with spaces") and unquoted (@path/file) mentions.
 */
export function extractAtMentionedFiles(content: string): string[] {
  const files = new Set<string>();

  // Match quoted paths: @"path with spaces"
  for (const match of content.matchAll(QUOTED_AT_MENTION_RE)) {
    files.add(match[2]);
  }

  // Match unquoted paths: @path/file
  for (const match of content.matchAll(AT_MENTION_RE)) {
    const raw = match[2];
    // Skip if it looks like an agent mention (@writer, @help, etc.)
    if (!raw.includes("/") && !raw.startsWith(".") && !raw.startsWith("/") && !raw.startsWith("~")) {
      continue;
    }
    files.add(raw);
  }

  return Array.from(files);
}

/**
 * Strip @-mentions from user text, leaving only the readable message.
 */
export function stripAtMentions(content: string): string {
  // Replace quoted mentions
  let result = content.replace(QUOTED_AT_MENTION_RE, "$1");
  // Replace unquoted mentions
  result = result.replace(AT_MENTION_RE, "$1");
  // Clean up extra whitespace
  return result.replace(/\s+/g, " ").trim();
}

// -- File Type Detection -------------------------------------------------------

function getExtension(path: string): string {
  const dot = path.lastIndexOf(".");
  if (dot === -1) return "";
  return path.slice(dot).toLowerCase();
}

function isTextFile(path: string): boolean {
  return TEXT_EXTENSIONS.has(getExtension(path));
}

function isImageFile(path: string): boolean {
  return IMAGE_EXTENSIONS.has(getExtension(path));
}

function getMimeType(path: string): string {
  const ext = getExtension(path);
  const mimeMap: Record<string, string> = {
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".gif": "image/gif",
    ".svg": "image/svg+xml",
    ".webp": "image/webp",
    ".bmp": "image/bmp",
    ".ico": "image/x-icon",
  };
  return mimeMap[ext] || "application/octet-stream";
}

// -- File Reading --------------------------------------------------------------

function readTextFile(absPath: string, lineRange?: { startLine?: number; endLine?: number }): { content: string; numLines: number; truncated: boolean } {
  const raw = readFileSync(absPath, "utf-8");
  const lines = raw.split("\n");
  const totalLines = lines.length;

  let content: string;
  let truncated = false;

  if (lineRange?.startLine !== undefined) {
    // Extract specific line range (1-indexed)
    const start = Math.max(0, lineRange.startLine - 1);
    const end = lineRange.endLine !== undefined 
      ? Math.min(lineRange.endLine, totalLines)
      : Math.min(start + MAX_LINES_TRUNCATE, totalLines);
    content = lines.slice(start, end).join("\n");
    truncated = end - start >= MAX_LINES_TRUNCATE;
  } else if (totalLines > MAX_LINES_TRUNCATE) {
    // Truncate large files
    content = lines.slice(0, MAX_LINES_TRUNCATE).join("\n");
    truncated = true;
  } else {
    content = raw;
  }

  // Also enforce byte size limit
  if (content.length > MAX_TEXT_SIZE) {
    content = content.slice(0, MAX_TEXT_SIZE);
    truncated = true;
  }

  return { content, numLines: totalLines, truncated };
}

function readImageFile(absPath: string): { data: string; size: number } {
  const stats = statSync(absPath);
  if (stats.size > MAX_IMAGE_SIZE) {
    throw new Error(`Image too large: ${stats.size} bytes (max ${MAX_IMAGE_SIZE})`);
  }

  const buffer = readFileSync(absPath);
  const data = buffer.toString("base64");
  return { data, size: stats.size };
}

// -- Main Parser ---------------------------------------------------------------

/**
 * Parse user input for @file mentions, resolve paths, read content,
 * and return cleaned text + attachment list.
 *
 * @param input - Raw user input (may contain @path/file references)
 * @param baseDir - Working directory for resolving relative paths
 * @returns { text: cleanedText, attachments: [...] }
 */
export function parseAttachments(input: string, baseDir: string = process.cwd()): ParseResult {
  const mentions = extractAtMentionedFiles(input);
  const attachments: Attachment[] = [];
  const errors: string[] = [];

  for (const mention of mentions) {
    // Check for line range
    const lineRange = parseLineRange(mention);
    const pathPart = lineRange?.path ?? mention;

    // Resolve path
    const absPath = expandPath(pathPart, baseDir);

    // Security: ensure path is within workspace or home
    const realPath = absPath; // In production, use fs.realpathSync for symlinks
    if (!realPath.startsWith(baseDir) && !realPath.startsWith(homedir())) {
      errors.push(`Path outside workspace: ${mention}`);
      continue;
    }

    // Check existence
    if (!existsSync(absPath)) {
      errors.push(`File not found: ${mention}`);
      continue;
    }

    // Check if supported type
    if (!isTextFile(pathPart) && !isImageFile(pathPart)) {
      errors.push(`Unsupported file type: ${mention}`);
      continue;
    }

    try {
      if (isTextFile(pathPart)) {
        const { content, numLines, truncated } = readTextFile(absPath, lineRange ?? undefined);
        attachments.push({
          type: "file",
          path: pathPart,
          absPath,
          content,
          numLines,
          truncated,
          mimeType: "text/plain",
        });
      } else if (isImageFile(pathPart)) {
        const { data, size } = readImageFile(absPath);
        attachments.push({
          type: "image",
          path: pathPart,
          absPath,
          data,
          mediaType: getMimeType(pathPart),
          size,
        });
      }
    } catch (e) {
      errors.push(`Error reading ${mention}: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  // Strip @mentions from visible text
  const text = stripAtMentions(input);

  return { text, attachments };
}

// -- Fake Tool Use/Result Generation (Mythos-style) ----------------------------

/**
 * Generate fake tool_use/tool_result pairs for attachments.
 * The LLM sees these as if it had called file_read itself.
 *
 * For text files:
 *   assistant: <tool_use>file_read({"file_path": "path"})</tool_use>
 *   tool: <tool_result>...content...</tool_result>
 *
 * For images:
 *   assistant: <tool_use>file_read({"file_path": "path"})</tool_use>
 *   tool: <tool_result><image type="image/png">base64data</image></tool_result>
 */
export interface FakeToolMessage {
  role: "assistant" | "tool";
  content: string;
}

export function generateFakeToolPairs(attachments: Attachment[]): FakeToolMessage[][] {
  const pairs: FakeToolMessage[][] = [];

  for (const att of attachments) {
    if (att.type === "file") {
      const toolUse: FakeToolMessage = {
        role: "assistant",
        content: `<tool_use>file_read({"file_path": "${att.path}"})</tool_use>`,
      };
      
      let content = att.content;
      if (att.truncated) {
        content += `\n\n[File truncated: showed ${MAX_LINES_TRUNCATE} of ${att.numLines} lines, or ${MAX_TEXT_SIZE} bytes]`;
      }
      
      const toolResult: FakeToolMessage = {
        role: "tool",
        content: `<tool_result>${content}</tool_result>`,
      };
      
      pairs.push([toolUse, toolResult]);
    } else if (att.type === "image") {
      const toolUse: FakeToolMessage = {
        role: "assistant",
        content: `<tool_use>file_read({"file_path": "${att.path}"})</tool_use>`,
      };
      
      const toolResult: FakeToolMessage = {
        role: "tool",
        content: `<tool_result><image type="${att.mediaType}" size="${att.size}">${att.data}</image></tool_result>`,
      };
      
      pairs.push([toolUse, toolResult]);
    }
  }

  return pairs;
}

// -- Utility -------------------------------------------------------------------

/**
 * Format attachment for display in the REPL (like Claude Code's dim lines).
 */
export function formatAttachmentSummary(att: Attachment): string {
  if (att.type === "file") {
    const size = att.content.length;
    const sizeStr = size > 1024 ? `${(size / 1024).toFixed(1)} KB` : `${size} B`;
    const truncation = att.truncated ? " (truncated)" : "";
    return `Read ${att.path} (${att.numLines} lines, ${sizeStr}${truncation})`;
  } else {
    const sizeStr = att.size > 1024 ? `${(att.size / 1024).toFixed(1)} KB` : `${att.size} B`;
    return `Read ${att.path} (${sizeStr})`;
  }
}
