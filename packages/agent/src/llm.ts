import Anthropic from "@anthropic-ai/sdk";
import type { AgentConfig } from "./config.js";

export class LLMClient {
  private client: Anthropic;
  private model: string;
  private maxTokens: number;

  constructor(config: AgentConfig) {
    this.client = new Anthropic({
      apiKey: config.api_key,
      baseURL: config.api_url,
    });
    this.model = config.model;
    this.maxTokens = config.max_tokens;
  }

  async generateBeat(
    systemPrompt: string,
    userMessage: string
  ): Promise<string> {
    const response = await this.client.messages.create({
      model: this.model,
      max_tokens: this.maxTokens,
      system: systemPrompt,
      messages: [{ role: "user", content: userMessage }],
    });

    const text = response.content
      .filter((b) => b.type === "text")
      .map((b) => b.text)
      .join("");

    return text;
  }
}
