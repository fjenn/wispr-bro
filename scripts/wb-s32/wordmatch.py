# SPDX-License-Identifier: GPL-3.0-or-later
"""The whisper practice's word match (S-32), for the Mac driver.

Mirrors `src/fork/tutorials/lessons/whisperMatch.ts`: words compared without
case, accents or punctuation ("I'm" = "im"), each heard word counts once,
extra words cost nothing; Han/Hiragana/Katakana runs compare per character.
PASS is >= 0.70 of the sentence's words. Change both together.

usage: python3 -I wordmatch.py <sentence> <heard>   (prints the share, exits
       0 when it passes, 1 when it does not)
"""
import re
import sys
import unicodedata

PASS = 0.70
_UNSPACED = re.compile(r"[぀-ヿ㐀-䶿一-鿿豈-﫿]")


def words(text: str) -> list[str]:
    plain = unicodedata.normalize("NFKD", text)
    plain = "".join(c for c in plain if not unicodedata.category(c).startswith("M"))
    plain = re.sub(r"['’ʼ]", "", plain.lower())
    runs = re.findall(r"[^\W_]+", plain)
    out: list[str] = []
    for run in runs:
        out.extend(list(run) if _UNSPACED.search(run) else [run])
    return out


def share(target: str, heard: str) -> float:
    want = words(target)
    if not want:
        return 0.0
    have: dict[str, int] = {}
    for w in words(heard):
        have[w] = have.get(w, 0) + 1
    found = 0
    for w in want:
        if have.get(w, 0) > 0:
            found += 1
            have[w] -= 1
    return found / len(want)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print(__doc__.strip().splitlines()[-2], file=sys.stderr)
        sys.exit(2)
    s = share(sys.argv[1], sys.argv[2])
    print(f"{s:.3f}")
    sys.exit(0 if s >= PASS else 1)
