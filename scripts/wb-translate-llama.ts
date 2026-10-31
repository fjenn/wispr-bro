// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The local `llama-server` backend of `wb-translate-missing.ts` (S-10, W-25).
 *
 *   bun scripts/wb-translate-missing.ts --backend llama-server \
 *     --server <path to llama-server> --model <gguf>
 *
 * Starts the pinned sidecar (built by `ci/llama-server/build.sh`) on a Unix
 * socket in a fresh temp dir with a random API key, `--offline` and a
 * scrubbed environment, exactly like the app does, and translates each
 * missing string with a JSON-schema reply. Nothing leaves the Mac. Results
 * are flagged "machine" (a person reviews them before S-37); a reply whose
 * `{{placeholders}}` differ from English is rejected by the caller and the
 * English copy is kept, flagged "untranslated".
 *
 * A general instruct model translates better than the cleanup fine-tune; the
 * RS-07 set has Qwen3-4B-Instruct-2507 Q4_K_M, which is what this was run
 * with for S-10.
 */
import { spawn, type Subprocess } from "bun";
import { randomBytes } from "crypto";
import { mkdtempSync, rmSync } from "fs";
import { tmpdir } from "os";
import path from "path";
import { LANGUAGE_METADATA } from "../src/i18n/languages";

export type Flag = "untranslated" | "machine";

export interface TranslationBackendLike {
  readonly name: string;
  readonly flag: Flag;
  translate(
    text: string,
    targetLang: string,
    key: string,
  ): Promise<string | null>;
  close?(): Promise<void>;
}

/** Sends one chat request; returns the message content or null. */
export type Complete = (
  body: Record<string, unknown>,
) => Promise<string | null>;

const RESPONSE_FORMAT = {
  type: "json_schema",
  json_schema: {
    name: "translation",
    strict: true,
    schema: {
      type: "object",
      properties: { text: { type: "string" } },
      required: ["text"],
      additionalProperties: false,
    },
  },
} as const;

export function languageName(code: string): string {
  return LANGUAGE_METADATA[code]?.name ?? code;
}

/** The system prompt for one target language. */
export function systemPrompt(code: string): string {
  return [
    `You translate user-interface strings of a macOS dictation app from English to ${languageName(code)}.`,
    "Rules:",
    "- Translate the meaning naturally for a settings screen; keep it short.",
    "- Keep every {{placeholder}} exactly as written, untranslated, including the braces.",
    "- Keep product and model names (Wispr Bro, Muesli, Qwen, Hugging Face, macOS, Mac) unchanged.",
    "- Keep symbols such as %, …, ·, and keyboard names unchanged.",
    "- If the string is a deliberately messy spoken sample (fillers, repeats, no punctuation), translate it as an equally messy spoken sample in the target language.",
    'Reply only with JSON: {"text": "<the translation>"}.',
  ].join("\n");
}

export function requestBody(
  text: string,
  code: string,
  key: string,
): Record<string, unknown> {
  return {
    model: "local",
    temperature: 0,
    seed: 42,
    max_tokens: Math.min(1024, 64 + text.length * 3),
    stream: false,
    response_format: RESPONSE_FORMAT,
    chat_template_kwargs: { enable_thinking: false },
    messages: [
      { role: "system", content: systemPrompt(code) },
      {
        role: "user",
        content: `String id: ${key}\nEnglish: ${JSON.stringify(text)}`,
      },
    ],
  };
}

/** Parses `{"text": …}`; null for anything else or an empty string. */
export function parseTranslation(content: string | null): string | null {
  if (!content) return null;
  try {
    const v = JSON.parse(content.trim()) as { text?: unknown };
    return typeof v.text === "string" && v.text.trim() ? v.text : null;
  } catch {
    return null;
  }
}

/** A backend over any `Complete` (tests pass a fake). */
export function backendFrom(
  complete: Complete,
  close?: () => Promise<void>,
): TranslationBackendLike {
  return {
    name: "llama-server",
    flag: "machine",
    async translate(text, targetLang, key) {
      try {
        return parseTranslation(
          await complete(requestBody(text, targetLang, key)),
        );
      } catch {
        return null;
      }
    },
    close,
  };
}

/** The spawn line the app uses (fork/llm/process.rs), minus the alias. */
export function serverArgs(model: string, socket: string): string[] {
  return [
    "-m",
    model,
    "--alias",
    "local",
    "--host",
    socket,
    "-c",
    "4096",
    "-np",
    "1",
    "--cache-ram",
    "256",
    "--reasoning",
    "off",
    "--jinja",
    "--no-ui",
    "--no-slots",
    "--offline",
  ];
}

/** Starts the sidecar and returns a backend that talks to it. */
export async function startLlamaServerBackend(
  server: string,
  model: string,
): Promise<TranslationBackendLike> {
  const dir = mkdtempSync(path.join(tmpdir(), "wb-translate-"));
  const socket = path.join(dir, "llm.sock");
  const key = randomBytes(32).toString("hex");
  const proc: Subprocess = spawn([server, ...serverArgs(model, socket)], {
    env: {
      PATH: "/usr/bin:/bin",
      HOME: process.env.HOME ?? dir,
      LLAMA_API_KEY: key,
    },
    stdout: "ignore",
    stderr: "ignore",
  });
  const headers = {
    authorization: `Bearer ${key}`,
    "content-type": "application/json",
  };
  const close = async () => {
    proc.kill("SIGTERM");
    await proc.exited;
    rmSync(dir, { recursive: true, force: true });
  };
  const deadline = Date.now() + 120_000;
  for (;;) {
    if (proc.exitCode !== null) {
      await close();
      throw new Error(`llama-server exited with ${proc.exitCode}`);
    }
    try {
      const r = await fetch("http://localhost/health", {
        unix: socket,
        headers,
      } as RequestInit);
      if (r.ok) break;
    } catch {
      // not listening yet
    }
    if (Date.now() > deadline) {
      await close();
      throw new Error("llama-server did not become healthy");
    }
    await Bun.sleep(100);
  }
  const complete: Complete = async (body) => {
    const r = await fetch("http://localhost/v1/chat/completions", {
      unix: socket,
      method: "POST",
      headers,
      body: JSON.stringify(body),
    } as RequestInit);
    if (!r.ok) return null;
    const j = (await r.json()) as {
      choices?: { finish_reason?: string; message?: { content?: string } }[];
    };
    const c = j.choices?.[0];
    if (!c || c.finish_reason === "length") return null;
    return c.message?.content ?? null;
  };
  return backendFrom(complete, close);
}
