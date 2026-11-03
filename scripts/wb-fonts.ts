// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * wb-fonts (S-13, Design Phase 0): vendor the Hub's fonts into
 * `src/assets/fonts/fork/` (R4 "Fonts", C-21, SYSTEM-PLAN §2). Fonts are
 * never fetched at runtime; the app ships these files.
 *
 *   bun scripts/wb-fonts.ts           # copy from node_modules, write OFL texts
 *   bun scripts/wb-fonts.ts --check   # verify the vendored files (CI)
 *
 * Sources are the Fontsource packages pinned in package.json devDependencies
 * (`@fontsource-variable/{figtree,inter,eb-garamond,jetbrains-mono}` 5.3.0,
 * all OFL-1.1). Fontsource already splits each family into per-script woff2
 * subsets with a matching `unicode-range`, so this script copies the subsets
 * we need instead of re-subsetting (`src/fork/styles/fonts.css` declares
 * them):
 *
 * - Figtree (variable weight, upright): latin, latin-ext. UI text.
 * - Inter (variable weight, upright): cyrillic(-ext), greek(-ext),
 *   vietnamese. The UI fallback for scripts Figtree lacks.
 * - EB Garamond (variable weight, upright and italic): latin(-ext),
 *   cyrillic(-ext), greek, vietnamese. Display moments only.
 * - JetBrains Mono (variable weight, upright): latin(-ext), cyrillic, greek.
 *   The typing test.
 *
 * Every other script falls back to the system fonts. Each file is pinned by
 * sha256: a package bump that changes a file fails `--check` until this
 * table is updated on purpose.
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
    pkg: "@fontsource-variable/figtree",
    file: "figtree-latin-wght-normal.woff2",
    sha256: "4ba7d3d096695818fe0686be4f1e82c6b05134e18a22260336130335027462dd",
  },
  {
    pkg: "@fontsource-variable/figtree",
    file: "figtree-latin-ext-wght-normal.woff2",
    sha256: "bf7828e2c258cffcfa50a048ee388a36b95bc16b452e8d36fa797635dbe15965",
  },
  {
    pkg: "@fontsource-variable/inter",
    file: "inter-cyrillic-wght-normal.woff2",
    sha256: "71d5ee93cc1e9f1d520a3a8b66456de18c7879d8df09d57fcd2eaff75fef0075",
  },
  {
    pkg: "@fontsource-variable/inter",
    file: "inter-cyrillic-ext-wght-normal.woff2",
    sha256: "ca157063339ac4ad418f214f3abfed119b0798ab4d377386ce5c9e5a7a435ebd",
  },
  {
    pkg: "@fontsource-variable/inter",
    file: "inter-greek-wght-normal.woff2",
    sha256: "1be3448e292fbf05ffe176fe1e43f135013d50b1e7d324ad1a558f623d3bb6f6",
  },
  {
    pkg: "@fontsource-variable/inter",
    file: "inter-greek-ext-wght-normal.woff2",
    sha256: "6e9e020a25f9b56d418f2c085b1d3c09725a4da23fe693a5b463064606732190",
  },
  {
    pkg: "@fontsource-variable/inter",
    file: "inter-vietnamese-wght-normal.woff2",
    sha256: "5c66f9e07e90c6d4ac4922cc68d60de26c17b1858e677fb5e603fce3952b3ff2",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    file: "eb-garamond-latin-wght-normal.woff2",
    sha256: "88603384163d301aebd5bd769832a8bfe3c9004ec24178417b9817f0ad32c63b",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    file: "eb-garamond-latin-ext-wght-normal.woff2",
    sha256: "f8d3fa06e20989a938972165406a1b731cdc7fe33ff2b0b6798c75aa63709901",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    file: "eb-garamond-cyrillic-wght-normal.woff2",
    sha256: "197ed98b8524dd8664aefb5d5ef4ca515d2df1ef90d3f771e19cb0893b95cc2b",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    file: "eb-garamond-cyrillic-ext-wght-normal.woff2",
    sha256: "7738b6004721f3d23e4e284b1055120f0ac1fd96fd01e5786b58b3042998876c",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    file: "eb-garamond-greek-wght-normal.woff2",
    sha256: "f6475b2f7530f248b5d3a0237d97dc2e1987665153dcb888fbeea3b0f9019b56",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    file: "eb-garamond-vietnamese-wght-normal.woff2",
    sha256: "af68d0b256558e0fecffabca6696d47c78902249db6bc7e1ac9964980a610a77",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    file: "eb-garamond-latin-wght-italic.woff2",
    sha256: "c664773f7e1d42d36326aa457becf7674236168c74cf7d74e87e5778ef7640f9",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    file: "eb-garamond-latin-ext-wght-italic.woff2",
    sha256: "fa866520f8b7fb646ebc730fd0d3cbf8945db74d92f25fdd6fee486808652fdf",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    file: "eb-garamond-cyrillic-wght-italic.woff2",
    sha256: "640020b01622b1c76c8f4137a3f39608fa22d718e17cb3e1d972e69c755ff6e0",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    file: "eb-garamond-cyrillic-ext-wght-italic.woff2",
    sha256: "5a7499fae103446ed709a2aaab7dbabb13ad1841a5258c0dd5b56750067ac2f1",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    file: "eb-garamond-greek-wght-italic.woff2",
    sha256: "288deb73f6a27a306138997d310e4efbfba9bb623eca0829be0e4475ff164b71",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    file: "eb-garamond-vietnamese-wght-italic.woff2",
    sha256: "b086ea6d8bc9bc670c11015f13df0776c430b6c26a78a73b4dce9ddb423623c1",
  },
  {
    pkg: "@fontsource-variable/jetbrains-mono",
    file: "jetbrains-mono-latin-wght-normal.woff2",
    sha256: "18be452724bfdc236c074ca94a249a7f41a86752c7d04ab258ce9ed5651f6a7e",
  },
  {
    pkg: "@fontsource-variable/jetbrains-mono",
    file: "jetbrains-mono-latin-ext-wght-normal.woff2",
    sha256: "79bfdab9ba467e26eea4122e6f2567e188dd8a09a8c730d501fc487c4ab99c6e",
  },
  {
    pkg: "@fontsource-variable/jetbrains-mono",
    file: "jetbrains-mono-cyrillic-wght-normal.woff2",
    sha256: "d274604c40757f98f07df623a8b8fb5194508a6fa4f5413a7a107f09e3b7452a",
  },
  {
    pkg: "@fontsource-variable/jetbrains-mono",
    file: "jetbrains-mono-greek-wght-normal.woff2",
    sha256: "ee863198077f3093ffa81c072581cc4bd608a04e60a8a9c0b846396ce4f14109",
  },
];

/**
 * OFL texts shipped in `LICENSES/` (NOTICE §7), in the same shape as
 * `LICENSES/OFL-1.1-NotoSansCJK.txt`: what is shipped, the copyright line from
 * the font's name table (ID 0), where the text comes from, then the package's
 * licence file unchanged. The copyright line is read from the vendored font
 * (`woff2Copyright`): Fontsource's packaged LICENSE files do not always name
 * the font's own copyright holder.
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
    pkg: "@fontsource-variable/figtree",
    version: "5.3.0",
    out: "OFL-1.1-Figtree.txt",
    title:
      "Figtree (variable weight, upright; latin and latin-ext subsets shipped in src/assets/fonts/fork/)",
    font: "figtree-latin-wght-normal.woff2",
  },
  {
    pkg: "@fontsource-variable/inter",
    version: "5.3.0",
    out: "OFL-1.1-Inter.txt",
    title:
      "Inter (variable weight, upright; cyrillic, cyrillic-ext, greek, greek-ext and vietnamese subsets shipped in src/assets/fonts/fork/)",
    font: "inter-greek-wght-normal.woff2",
  },
  {
    pkg: "@fontsource-variable/eb-garamond",
    version: "5.3.0",
    out: "OFL-1.1-EBGaramond.txt",
    title:
      "EB Garamond (variable weight, upright and italic; latin, latin-ext, cyrillic, cyrillic-ext, greek and vietnamese subsets shipped in src/assets/fonts/fork/)",
    font: "eb-garamond-latin-wght-normal.woff2",
  },
  {
    pkg: "@fontsource-variable/jetbrains-mono",
    version: "5.3.0",
    out: "OFL-1.1-JetBrainsMono.txt",
    title:
      "JetBrains Mono (variable weight, upright; latin, latin-ext, cyrillic and greek subsets shipped in src/assets/fonts/fork/)",
    font: "jetbrains-mono-latin-wght-normal.woff2",
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
