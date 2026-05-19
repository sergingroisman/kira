import type { tool } from "@langchain/core/tools";

// ToolDefinition is the plugin contract.
// Phase 2 will add: loadFromDir(dir: string): Promise<void> to the class.
// Phase 3 (MCP) will register MCP-sourced tools via the same register() method.
export interface ToolDefinition {
  name: string;
  lcTool: ReturnType<typeof tool>;
}

export class ToolRegistry {
  private readonly tools: ToolDefinition[] = [];

  register(def: ToolDefinition): void {
    if (this.tools.some(t => t.name === def.name)) {
      throw new Error(`Tool "${def.name}" is already registered`);
    }
    this.tools.push(def);
  }

  getLCTools(): ReturnType<typeof tool>[] {
    return this.tools.map(d => d.lcTool);
  }

  getNames(): string[] {
    return this.tools.map(d => d.name);
  }
}

// Singleton registry — initialized at startup, stable reference for lifetime of process
export const registry = new ToolRegistry();
