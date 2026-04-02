import { readCard } from "./elore.js";
import { readdirSync } from "fs";
import { join } from "path";

/** Extract entity IDs mentioned in elore suggest output. */
export function extractEntities(suggestOutput: string): string[] {
  // Match entity IDs from tension predicates like armed_danger(kian, nova)
  const ids = new Set<string>();
  const re = /\b([a-z_][a-z0-9_]*)\b/g;
  let m;
  while ((m = re.exec(suggestOutput))) {
    // Filter out known predicate names
    const skip = new Set([
      "threatens", "armed_danger", "active_danger", "protector",
      "pressure_to_harm", "pressure_to_spare", "torn",
      "enemy", "alliance_opportunity", "would_confide",
      "betrayal_opportunity", "dramatic_irony", "info_cascade",
      "suspense", "active_conflict", "plots_against",
      "can_meet", "personal_bond", "would_obey", "would_sacrifice",
      "power_advantage", "must_submit", "deceiving", "deceived",
      "critical_reveal", "orphaned_secret", "indirect_protector",
    ]);
    if (!skip.has(m[1])) {
      ids.add(m[1]);
    }
  }
  return [...ids];
}

/** Read entity cards for given IDs. */
export function fetchEntityCards(
  projectDir: string,
  entityIds: string[]
): string[] {
  const cards: string[] = [];
  const dirs = ["characters", "locations", "factions", "secrets"];

  for (const dir of dirs) {
    const fullDir = join(projectDir, "cards", dir);
    let files: string[];
    try {
      files = readdirSync(fullDir).filter((f) => f.endsWith(".md"));
    } catch {
      continue;
    }
    for (const file of files) {
      const id = file.replace(".md", "");
      if (entityIds.length === 0 || entityIds.includes(id)) {
        cards.push(readCard(projectDir, join("cards", dir, file)));
      }
    }
  }
  return cards;
}

/** Read recent beat cards for continuity. */
export function readRecentBeats(
  projectDir: string,
  phaseId: string,
  currentSeq: number,
  count: number = 3
): string[] {
  const beats: string[] = [];
  const dir = join(projectDir, "cards", "phases", phaseId);

  for (let i = Math.max(1, currentSeq - count); i < currentSeq; i++) {
    const file = join(dir, `${String(i).padStart(3, "0")}.md`);
    try {
      beats.push(readCard(projectDir, file));
    } catch {
      // beat doesn't exist yet
    }
  }
  return beats;
}
