// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for the local llama-server translation backend (S-10).
import { describe, expect, test } from "bun:test";
import {
  backendFrom,
  languageName,
  parseTranslation,
  requestBody,
  serverArgs,
  systemPrompt,
} from "./wb-translate-llama";

describe("wb-translate-llama", () => {
  test("request: JSON schema, no thinking, placeholder rule, target language", () => {
    const body = requestBody("Model: {{name}}", "de", "aiEngine.model");
    expect(body.temperature).toBe(0);
    expect(body.chat_template_kwargs).toEqual({ enable_thinking: false });
    expect((body.response_format as { type: string }).type).toBe("json_schema");
    const [system, user] = body.messages as { role: string; content: string }[];
    expect(system.role).toBe("system");
    expect(system.content).toContain("German");
    expect(system.content).toContain("{{placeholder}}");
    expect(user.content).toContain('"Model: {{name}}"');
    expect(user.content).toContain("aiEngine.model");
    expect(languageName("zh-TW")).toBe("Traditional Chinese");
    expect(languageName("xx")).toBe("xx");
    expect(systemPrompt("ja")).toContain("Japanese");
  });

  test("replies must be JSON with a non-empty text", () => {
    expect(parseTranslation('{"text":"Bereit"}')).toBe("Bereit");
    expect(parseTranslation("Bereit")).toBeNull();
    expect(parseTranslation('{"text":"  "}')).toBeNull();
    expect(parseTranslation('{"other":1}')).toBeNull();
    expect(parseTranslation(null)).toBeNull();
  });

  test("backend flags machine and falls back to null on errors", async () => {
    const seen: unknown[] = [];
    const b = backendFrom(async (body) => {
      seen.push(body);
      return '{"text":"Bereit"}';
    });
    expect(b.flag).toBe("machine");
    expect(await b.translate("Ready", "de", "aiEngine.state.ready")).toBe(
      "Bereit",
    );
    expect(seen).toHaveLength(1);
    const failing = backendFrom(async () => {
      throw new Error("socket closed");
    });
    expect(await failing.translate("Ready", "de", "k")).toBeNull();
  });

  test("spawn line is the app's: socket, offline, no UI, no slots, never -hf", () => {
    const args = serverArgs("/m.gguf", "/t/llm.sock");
    expect(args).toContain("--offline");
    expect(args).toContain("--no-ui");
    expect(args).toContain("--no-slots");
    expect(args[args.indexOf("--host") + 1]).toBe("/t/llm.sock");
    expect(args.some((a) => a.startsWith("-hf") || a.startsWith("--hf"))).toBe(
      false,
    );
  });
});
