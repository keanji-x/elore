import { resolve } from "path";
import { loadConfig } from "./config.js";
import { LLMClient } from "./llm.js";
import { runLoop } from "./loop.js";

async function main() {
  const args = process.argv.slice(2);
  const phaseId = args[0];
  const maxBeats = parseInt(args[1] ?? "1", 10);
  const direction = args[2];

  if (!phaseId) {
    console.error("Usage: elore-agent <phase_id> [max_beats] [direction]");
    console.error("Example: elore-agent act1 5 '推动基安进入尖塔'");
    process.exit(1);
  }

  const projectDir = resolve(process.cwd());
  const config = loadConfig(resolve(projectDir, "elore.toml"));

  console.log(`Model: ${config.agent.model}`);
  console.log(`Project: ${projectDir}`);
  console.log(`Phase: ${phaseId}, Max beats: ${maxBeats}`);

  const client = new LLMClient(config.agent);
  await runLoop(client, { projectDir, phaseId, maxBeats, direction });

  console.log("\nDone.");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
