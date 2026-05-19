import { describe, it, expect, afterEach } from "vitest";
import { writeFile, unlink, readFile } from "fs/promises";
import { fileReadTool } from "../../src/tools/fileRead.js";
import { fileWriteTool, fileAppendTool } from "../../src/tools/fileWrite.js";
import { tmpdir } from "os";
import { join } from "path";

const tmpPath = (name: string) => join(tmpdir(), `sergin-test-${name}-${Date.now()}.txt`);

describe("fileReadTool (AGENT-03)", () => {
  it("reads file contents", async () => {
    const path = tmpPath("read");
    await writeFile(path, "hello from test", "utf-8");
    const result = await fileReadTool.lcTool.invoke({ path });
    expect(result).toBe("hello from test");
    await unlink(path);
  });

  it("returns error string for missing file — does not throw", async () => {
    const result = await fileReadTool.lcTool.invoke({ path: "/tmp/nonexistent_xyz_123_abc.txt" });
    expect(result).toMatch(/Error: File not found/);
  });

  it("tool name is 'FileRead'", () => {
    expect(fileReadTool.name).toBe("FileRead");
  });
});

describe("fileWriteTool (AGENT-03)", () => {
  it("writes content to a file", async () => {
    const path = tmpPath("write");
    const result = await fileWriteTool.lcTool.invoke({ path, content: "written content" });
    expect(result).toMatch(/Written \d+ bytes to/);
    const actual = await readFile(path, "utf-8");
    expect(actual).toBe("written content");
    await unlink(path);
  });

  it("overwrites existing file", async () => {
    const path = tmpPath("overwrite");
    await writeFile(path, "old content", "utf-8");
    await fileWriteTool.lcTool.invoke({ path, content: "new content" });
    const actual = await readFile(path, "utf-8");
    expect(actual).toBe("new content");
    await unlink(path);
  });
});

describe("fileAppendTool (AGENT-03)", () => {
  it("appends to an existing file", async () => {
    const path = tmpPath("append");
    await writeFile(path, "line1\n", "utf-8");
    await fileAppendTool.lcTool.invoke({ path, content: "line2\n" });
    const actual = await readFile(path, "utf-8");
    expect(actual).toBe("line1\nline2\n");
    await unlink(path);
  });
});
