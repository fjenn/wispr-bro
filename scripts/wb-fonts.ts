// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * wb-fonts (S-13): vendor the Hub's display serif into
 * `src/assets/fonts/fork/` (R4 "Fonts", C-21). Fonts are never fetched at
 * runtime; the app ships these files.
 *
 *   bun scripts/wb-fonts.ts           # copy from node_modules, write OFL texts
 *   bun scripts/wb-fonts.ts --check   # verify the vendored files (CI)
 *
 * Sources are the Fontsource packages pinned in package.json devDependencies
 * (`@fontsource-variable/newsreader` 5.3.0, `@fontsource-variable/
 * source-serif-4` 5.3.1, both OFL-1.1). Fontsource already splits each family
 * into per-script woff2 subsets with a matching `unicode-range`, so this
 * script copies the subsets we need instead of re-subsetting:
 *
 * - Newsreader (variable weight, upright): latin, latin-ext, vietnamese. The
 *   display face for headings and banners.
 * - Source Serif 4 (variable weight, upright): cyrillic, cyrillic-ext, greek.
 *   Newsreader has no Cyrillic or Greek, so these scripts fall back to it
 *   (R4: "Cyrillic/Greek fallback").
 *
 * Every other script falls back to the system serif (`src/fork/styles/
 * fonts.css`). Each file is pinned by sha256: a package bump that changes a
 * file fails `--check` until this table is updated on purpose.
 */
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { brotliDecompressSync } from "node:zlib";

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUT = path.join(ROOT, "src/assets/fonts/fork");
const LICENSES = path.join(ROOT, "LICENSES");

type FontFile = { pkg: string; file: string; sha256: string };

export const FONT_FILES: FontFile[] = [
  {
    pkg: "@fontsource-variable/newsreader",
    file: "newsreader-latin-wght-normal.woff2",
    sha256: "62981321d9a3cc7a61a73792729043703fd6112da86e8ec848bb57f088578757",
  },
  {
    pkg: "@fontsource-variable/newsreader",
    file: "newsreader-latin-ext-wght-normal.woff2",
    sha256: "ac6fa9ed533278f4c8fd3ae44a1fc78c7df736040237ab86fc1160d020af0af2",
  },
  {
    pkg: "@fontsource-variable/newsreader",
    file: "newsreader-vietnamese-wght-normal.woff2",
    sha256: "f0dcc9795666850cc4cd8d50d55c44d48d9103cc86f552b4a12f0793eb915840",
  },
  {
    pkg: "@fontsource-variable/source-serif-4",
    file: "source-serif-4-cyrillic-wght-normal.woff2",
    sha256: "a81082707b85203d7f2ae33833f7ae0bf832db167c8710f92f805a9808fb051a",
  },
  {
    pkg: "@fontsource-variable/source-serif-4",
    file: "source-serif-4-cyrillic-ext-wght-normal.woff2",
    sha256: "da49e14ae1d459f3d04cf4555fb954f6e54b247b708f8cfe2d8e7e4ee3c6829b",
  },
  {
    pkg: "@fontsource-variable/source-serif-4",
    file: "source-serif-4-greek-wght-normal.woff2",
    sha256: "54c4728aa0fd97d59b89b6d89b230065794b4c088c55efe71abc9ce882cfa853",
  },
];

/**
 * OFL texts shipped in `LICENSES/` (NOTICE §7), in the same shape as
 * `LICENSES/OFL-1.1-NotoSansCJK.txt`: what is shipped, the copyright line from
 * the font's name table (ID 0), where the text comes from, then the package's
 * licence file unchanged. The copyright line is read from the vendored font
 * (`woff2Copyright`) because the Source Serif 4 package's LICENSE names only
 * "Google Inc." (Fontsource's packaging), while the font itself credits Adobe.
 */
export const LICENSE_FILES: {
  pkg: string;
  version: string;
  out: string;
  title: string;
  /** Vendored file whose name table gives the copyright line. */
  font: string;
}[] = [
  {
    pkg: "@fontsource-variable/newsreader",
    version: "5.3.0",
    out: "OFL-1.1-Newsreader.txt",
    title:
      "Newsreader (variable weight, upright; latin, latin-ext and vietnamese subsets shipped in src/assets/fonts/fork/)",
    font: "newsreader-latin-wght-normal.woff2",
  },
  {
    pkg: "@fontsource-variable/source-serif-4",
    version: "5.3.1",
    out: "OFL-1.1-SourceSerif4.txt",
    title:
      "Source Serif 4 (variable weight, upright; cyrillic, cyrillic-ext and greek subsets shipped in src/assets/fonts/fork/)",
    font: "source-serif-4-greek-wght-normal.woff2",
  },
];

/** Known table tags of the WOFF2 directory, by flag index (WOFF2 §5.1). */
const WOFF2_TAGS =
  "cmap head hhea hmtx maxp name OS/2 post cvt  fpgm glyf loca prep CFF  VORG EBDT EBLC gasp hdmx kern LTSH PCLT VDMX vhea vmtx BASE GDEF GPOS GSUB EBSC JSTF MATH CBDT CBLC COLR CPAL SVG  sbix acnt avar bdat bloc bsln cvar fdsc feat fmtx fvar gvar hsty just lcar mort morx opbd prop trak Zapf Silf Glat Gloc Feat Sill"
    .match(/.{4}\s?/g)!
    .map((t) => t.slice(0, 4));

/**
 * The copyright string (name ID 0, Windows platform) of a WOFF2 font. The
 * `name` table is never transformed in WOFF2, so it is read straight from
 * the decompressed table data.
 */
export function woff2Copyright(buf: Buffer): string {
  if (buf.toString("latin1", 0, 4) !== "wOF2") throw new Error("not WOFF2");
  const numTables = buf.readUInt16BE(12);
  const compressedLength = buf.readUInt32BE(20);
  let off = 48;
  const base128 = () => {
    let v = 0;
    for (let i = 0; i < 5; i++) {
      const b = buf[off++];
      v = v * 128 + (b & 0x7f);
      if (!(b & 0x80)) return v;
    }
    throw new Error("bad UIntBase128");
  };
  const tables: { tag: string; length: number }[] = [];
  for (let i = 0; i < numTables; i++) {
    const flags = buf[off++];
    let tag: string;
    if ((flags & 0x3f) === 0x3f) {
      tag = buf.toString("latin1", off, off + 4);
      off += 4;
    } else {
      tag = WOFF2_TAGS[flags & 0x3f];
    }
    const transform = flags >> 6;
    let length = base128();
    const glyfLoca = tag === "glyf" || tag === "loca";
    if (glyfLoca ? transform === 0 : transform !== 0) length = base128();
    tables.push({ tag, length });
  }
  const data = brotliDecompressSync(buf.subarray(off, off + compressedLength));
  let at = 0;
  for (const t of tables) {
    if (t.tag === "name") {
      const n = data.subarray(at, at + t.length);
      const count = n.readUInt16BE(2);
      const strings = n.readUInt16BE(4);
      for (let i = 0; i < count; i++) {
        const r = 6 + i * 12;
        if (n.readUInt16BE(r) !== 3 || n.readUInt16BE(r + 6) !== 0) continue;
        const len = n.readUInt16BE(r + 8);
        const start = strings + n.readUInt16BE(r + 10);
        const raw = n.subarray(start, start + len);
        let text = "";
        for (let j = 0; j + 1 < raw.length; j += 2) {
          text += String.fromCharCode(raw.readUInt16BE(j));
        }
        return text;
      }
    }
    at += t.length;
  }
  throw new Error("no copyright (name ID 0) in the font");
}

function licenceText(l: (typeof LICENSE_FILES)[number], body: string): string {
  const copyright = woff2Copyright(fs.readFileSync(path.join(OUT, l.font)));
  return [
    l.title,
    `Copyright line (font name table, ID 0): ${copyright}`,
    `Source: npm ${l.pkg}@${l.version}, file LICENSE (reproduced unchanged below).`,
    "",
    "------------------------",
    "",
    body,
  ].join("\n");
}

const sha256 = (buf: Buffer): string =>
  createHash("sha256").update(buf).digest("hex");

function main(): number {
  const check = process.argv.includes("--check");
  const problems: string[] = [];

  for (const f of FONT_FILES) {
    const out = path.join(OUT, f.file);
    if (check) {
      if (!fs.existsSync(out)) {
        problems.push(`missing ${path.relative(ROOT, out)}`);
        continue;
      }
      const got = sha256(fs.readFileSync(out));
      if (got !== f.sha256) {
        problems.push(
          `${path.relative(ROOT, out)} has sha256 ${got}, expected ${f.sha256}`,
        );
      }
      continue;
    }
    const src = path.join(ROOT, "node_modules", f.pkg, "files", f.file);
    const buf = fs.readFileSync(src);
    const got = sha256(buf);
    if (f.sha256 && got !== f.sha256) {
      problems.push(
        `${f.pkg}/${f.file} has sha256 ${got}, expected ${f.sha256}`,
      );
      continue;
    }
    fs.mkdirSync(OUT, { recursive: true });
    fs.writeFileSync(out, buf);
    console.log(`${path.relative(ROOT, out)}  ${got}`);
  }

  for (const l of LICENSE_FILES) {
    const out = path.join(LICENSES, l.out);
    if (check) {
      if (!fs.existsSync(out)) {
        problems.push(`missing LICENSES/${l.out}`);
      } else if (!fs.readFileSync(out, "utf8").startsWith(licenceText(l, ""))) {
        problems.push(`LICENSES/${l.out} does not start with its header`);
      }
      continue;
    }
    const src = path.join(ROOT, "node_modules", l.pkg, "LICENSE");
    fs.writeFileSync(out, licenceText(l, fs.readFileSync(src, "utf8")));
    console.log(`LICENSES/${l.out}`);
  }

  if (check) {
    const expected = new Set(FONT_FILES.map((f) => f.file));
    for (const name of fs.existsSync(OUT) ? fs.readdirSync(OUT) : []) {
      if (!expected.has(name)) {
        problems.push(`unexpected file src/assets/fonts/fork/${name}`);
      }
    }
  }

  for (const p of problems) console.error(`wb-fonts: ${p}`);
  if (problems.length === 0 && check) console.log("wb-fonts: ok");
  return problems.length === 0 ? 0 : 1;
}

if (import.meta.main) process.exit(main());
