// SPDX-License-Identifier: GPL-3.0-or-later
//
// Verifies a Tauri updater signature (`<artifact>.sig`) against a Tauri
// updater public key (`plugins.updater.pubkey`, or the `.pub` file), the same
// check tauri-plugin-updater runs before it installs anything (minisign:
// Ed25519 over the BLAKE2b-512 of the file, plus the global signature over the
// trusted comment). F-02 runs it on every release artifact in `macos.sh` and
// in the wb-verify job, so a release signed with the wrong key never ships.
//
// Usage: bun scripts/wb-release/minisign.ts <artifact> [<sig file>] [--pubkey <base64>]
// The pubkey defaults to the overlay's `plugins.updater.pubkey`.

import { createHash, createPublicKey, verify } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const REPO_ROOT = join(import.meta.dir, "../..");

export interface MinisignResult {
  ok: boolean;
  keyId?: string;
  trustedComment?: string;
  error?: string;
}

const b64 = (s: string) => Buffer.from(s.trim(), "base64");

/** Lines of a Tauri value: base64 of the minisign text file (or the text). */
function textOf(value: string): string[] {
  const raw = value.trim();
  const text = raw.startsWith("untrusted comment:")
    ? raw
    : b64(raw).toString("utf8");
  return text.split("\n").map((l) => l.replace(/\r$/, ""));
}

function keyIdOf(bytes: Buffer): string {
  return Buffer.from(bytes).reverse().toString("hex").toUpperCase();
}

function ed25519(pub: Buffer) {
  return createPublicKey({
    key: { kty: "OKP", crv: "Ed25519", x: pub.toString("base64url") },
    format: "jwk",
  });
}

export function verifyMinisign(
  data: Buffer,
  signature: string,
  pubkey: string,
): MinisignResult {
  const keyLines = textOf(pubkey);
  const key = b64(keyLines[1] ?? "");
  if (key.length !== 42 || key.subarray(0, 2).toString("latin1") !== "Ed") {
    return { ok: false, error: "public key is not a minisign Ed25519 key" };
  }
  const keyId = keyIdOf(key.subarray(2, 10));
  const lines = textOf(signature);
  const sig = b64(lines[1] ?? "");
  const trusted = lines[2]?.startsWith("trusted comment: ")
    ? lines[2].slice("trusted comment: ".length)
    : undefined;
  const global = b64(lines[3] ?? "");
  if (sig.length !== 74 || trusted === undefined || global.length !== 64) {
    return { ok: false, keyId, error: "malformed signature" };
  }
  const alg = sig.subarray(0, 2).toString("latin1");
  const sigKeyId = keyIdOf(sig.subarray(2, 10));
  if (sigKeyId !== keyId) {
    return {
      ok: false,
      keyId,
      error: `signed by key ${sigKeyId}, expected ${keyId}`,
    };
  }
  const message =
    alg === "ED"
      ? createHash("blake2b512").update(data).digest()
      : alg === "Ed"
        ? data
        : undefined;
  if (!message) return { ok: false, keyId, error: `unknown algorithm ${alg}` };
  const pub = ed25519(key.subarray(10));
  const raw = sig.subarray(10);
  if (!verify(null, message, pub, raw)) {
    return { ok: false, keyId, error: "signature does not match the file" };
  }
  const globalMessage = Buffer.concat([raw, Buffer.from(trusted, "utf8")]);
  if (!verify(null, globalMessage, pub, global)) {
    return { ok: false, keyId, error: "trusted comment signature is invalid" };
  }
  return { ok: true, keyId, trustedComment: trusted };
}

export function overlayPubkey(root = REPO_ROOT): string {
  const overlay = JSON.parse(
    readFileSync(join(root, "src-tauri/tauri.fork.conf.json"), "utf8"),
  );
  return overlay.plugins.updater.pubkey as string;
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const i = argv.indexOf("--pubkey");
  const pubkey = i >= 0 ? argv.splice(i, 2)[1] : overlayPubkey();
  const [artifact, sigFile = `${argv[0]}.sig`] = argv;
  if (!artifact) {
    console.error("usage: minisign.ts <artifact> [<sig>] [--pubkey <base64>]");
    process.exit(2);
  }
  const r = verifyMinisign(
    readFileSync(artifact),
    readFileSync(sigFile, "utf8"),
    pubkey,
  );
  if (r.ok) {
    console.log(`✓ ${artifact}: signed by updater key ${r.keyId}`);
  } else {
    console.error(`✗ ${artifact}: ${r.error}`);
    process.exit(1);
  }
}
