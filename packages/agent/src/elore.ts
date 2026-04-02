import { execFile } from "child_process";
import { readFileSync, writeFileSync, readdirSync } from "fs";
import { resolve, join } from "path";

/** Run an elore CLI command and return stdout. */
export function exec(
  command: string,
  args: string[] = [],
  cwd?: string
): Promise<string> {
  return new Promise((resolve, reject) => {
    execFile(command, args, { cwd }, (err, stdout, stderr) => {
      if (err) {
        reject(new Error(`${command} ${args.join(" ")} failed: ${stderr || err.message}`));
      } else {
        resolve(stdout);
      }
    });
  });
}

export async function eloreBuild(projectDir: string): Promise<string> {
  return exec("elore", ["build"], projectDir);
}

export async function eloreSuggest(projectDir: string): Promise<string> {
  return exec("elore", ["suggest"], projectDir);
}

export function readCard(projectDir: string, cardPath: string): string {
  return readFileSync(resolve(projectDir, cardPath), "utf-8");
}

export function writeBeatCard(
  projectDir: string,
  phaseId: string,
  seq: number,
  content: string
): void {
  const dir = join(projectDir, "cards", "phases", phaseId);
  const file = join(dir, `${String(seq).padStart(3, "0")}.md`);
  writeFileSync(file, content, "utf-8");
}

export function getNextSeq(projectDir: string, phaseId: string): number {
  const dir = join(projectDir, "cards", "phases", phaseId);
  try {
    const files = readdirSync(dir).filter((f) => f.endsWith(".md"));
    if (files.length === 0) return 1;
    const seqs = files.map((f) => parseInt(f.replace(".md", ""), 10)).filter(Number.isFinite);
    return Math.max(...seqs) + 1;
  } catch {
    return 1;
  }
}
