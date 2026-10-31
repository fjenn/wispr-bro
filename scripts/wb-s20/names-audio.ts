// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * S-20 name eval audio: one 16 kHz mono WAV per item of
 * `eval/context/names.jsonl`, spoken by macOS TTS **into a file** (`say -o`,
 * nothing is played), for `fork::context::names_eval`.
 *
 *   bun scripts/wb-s20/names-audio.ts <out dir> [voice]   # default Samantha
 */
import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";

const [outDir, voice = "Samantha"] = process.argv.slice(2);
if (!outDir) {
  console.error("usage: bun scripts/wb-s20/names-audio.ts <out dir> [voice]");
  process.exit(2);
}
fs.mkdirSync(outDir, { recursive: true });
const items = fs
  .readFileSync(
    path.join(import.meta.dir, "..", "..", "eval", "context", "names.jsonl"),
    "utf8",
  )
  .split("\n")
  .filter((l) => l.trim())
  .map((l) => JSON.parse(l) as { id: string; said: string });

function run(cmd: string, args: string[]) {
  const r = spawnSync(cmd, args, { stdio: "inherit" });
  if (r.status !== 0) throw new Error(`${cmd} failed`);
}

for (const item of items) {
  const aiff = path.join(outDir, `${item.id}.aiff`);
  const wav = path.join(outDir, `${item.id}.wav`);
  run("/usr/bin/say", ["-v", voice, "-o", aiff, item.said]);
  run("/usr/bin/afconvert", ["-f", "WAVE", "-d", "LEI16@16000", "-c", "1", aiff, wav]);
  fs.rmSync(aiff);
}
console.log(`${items.length} WAVs in ${outDir} (${voice})`);
