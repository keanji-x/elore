import { readFileSync } from "fs";
import { resolve } from "path";
import TOML from "toml";

export interface AgentConfig {
  api_url: string;
  api_key: string;
  model: string;
  max_tokens: number;
}

export interface EloreConfig {
  server?: {
    host?: string;
    port?: number;
    data_dir?: string;
  };
  agent: AgentConfig;
}

const DEFAULTS: AgentConfig = {
  api_url: "https://api.anthropic.com",
  api_key: "",
  model: "claude-sonnet-4-6-20250514",
  max_tokens: 4096,
};

export function loadConfig(configPath?: string): EloreConfig {
  const file = configPath ?? resolve(process.cwd(), "elore.toml");

  let raw: Record<string, unknown> = {};
  try {
    raw = TOML.parse(readFileSync(file, "utf-8"));
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e;
    // no config file — use defaults + env
  }

  const fileAgent = (raw.agent ?? {}) as Partial<AgentConfig>;

  const agent: AgentConfig = {
    api_url: env("ELORE_API_URL") ?? fileAgent.api_url ?? DEFAULTS.api_url,
    api_key: env("ELORE_API_KEY") ?? fileAgent.api_key ?? DEFAULTS.api_key,
    model: env("ELORE_MODEL") ?? fileAgent.model ?? DEFAULTS.model,
    max_tokens: fileAgent.max_tokens ?? DEFAULTS.max_tokens,
  };

  if (!agent.api_key) {
    throw new Error(
      "API key not configured. Set [agent].api_key in elore.toml or ELORE_API_KEY env var."
    );
  }

  return { server: raw.server as EloreConfig["server"], agent };
}

function env(key: string): string | undefined {
  const v = process.env[key];
  return v && v.length > 0 ? v : undefined;
}
