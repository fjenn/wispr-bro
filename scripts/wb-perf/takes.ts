// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * S-38 latency bench audio: one 16 kHz mono WAV per take of
 * `eval/perf/takes.jsonl`, spoken by macOS TTS **into a file** (`say -o`,
 * nothing is played), for `fork::perf_bench`.
 *
 *   bun scripts/wb-perf/takes.ts <out dir> [voice]   # default Samantha
 */
import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";

const [outDir, voice = "Samantha"] = process.argv.slice(2);
if (!outDir) {
  console.error("usage: bun scripts/wb-perf/takes.ts <out dir> [voice]");
  process.exit(2);
}
fs.mkdirSync(outDir, { recursive: true });
const takes = fs
  .readFileSync(
    path.join(import.meta.dir, "..", "..", "eval", "perf", "takes.jsonl"),
    "utf8",
  )
  .split("\n")
  .filter((l) => l.trim())
  .map((l) => JSON.parse(l) as { id: string; text: string });

function run(cmd: string, args: string[]) {
  const r = spawnSync(cmd, args, { stdio: "inherit" });
  if (r.status !== 0) throw new Error(`${cmd} failed`);
}

for (const take of takes) {
  const wav = path.join(outDir, `${take.id}.wav`);
  if (fs.existsSync(wav)) continue;
  const aiff = path.join(outDir, `${take.id}.aiff`);
  run("/usr/bin/say", ["-v", voice, "-o", aiff, take.text]);
  run("/usr/bin/afconvert", [
    "-f",
    "WAVE",
    "-d",
    "LEI16@16000",
    "-c",
    "1",
    aiff,
    wav,
  ]);
  fs.rmSync(aiff);
}
console.log(`${takes.length} WAVs in ${outDir} (${voice})`);
