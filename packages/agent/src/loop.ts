import { LLMClient } from "./llm.js";
import { eloreBuild, eloreSuggest, writeBeatCard, getNextSeq } from "./elore.js";
import { extractEntities, fetchEntityCards, readRecentBeats } from "./context.js";
import { SYSTEM_PROMPT, buildUserMessage } from "./prompt.js";

export interface LoopOptions {
  projectDir: string;
  phaseId: string;
  maxBeats?: number;
  direction?: string;
}

export async function runLoop(client: LLMClient, opts: LoopOptions) {
  const { projectDir, phaseId, maxBeats = 1 } = opts;

  for (let i = 0; i < maxBeats; i++) {
    const seq = getNextSeq(projectDir, phaseId);
    console.log(`\n--- Beat ${seq} ---`);

    // ① Build to sync world state
    console.log("Building...");
    await eloreBuild(projectDir);

    // ② Get tensions from reasoning engine
    console.log("Analyzing tensions...");
    let worldState: string;
    try {
      worldState = await eloreSuggest(projectDir);
    } catch {
      worldState = "(suggest unavailable)";
    }

    // ③ Pre-fetch entity cards (anti-hallucination)
    const entityIds = extractEntities(worldState);
    const entityCards = fetchEntityCards(projectDir, entityIds);
    console.log(`Pre-fetched ${entityCards.length} entity cards`);

    // ④ Read recent beats for continuity
    const recentBeats = readRecentBeats(projectDir, phaseId, seq);

    // ⑤ Generate beat via LLM
    console.log("Generating...");
    const userMessage = buildUserMessage({
      worldState,
      entityCards,
      recentBeats,
      seq,
      direction: opts.direction,
    });

    const output = await client.generateBeat(SYSTEM_PROMPT, userMessage);

    // ⑥ Write beat card
    writeBeatCard(projectDir, phaseId, seq, output);
    console.log(`Wrote cards/phases/${phaseId}/${String(seq).padStart(3, "0")}.md`);

    // ⑦ Rebuild to trigger reverse sync
    console.log("Rebuilding (reverse sync)...");
    await eloreBuild(projectDir);

    console.log(`Beat ${seq} done.`);
  }
}
