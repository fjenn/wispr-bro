# SPDX-License-Identifier: GPL-3.0-or-later
"""Makes the S-32 "whispered take at 30 cm" WAV from a `say -o` file.

A synthetic stand-in, never recorded or played aloud: the input is a 16 kHz
mono 16-bit WAV from `say` (the Whisper voice when the Mac has it), scaled to
GAIN_DB (default -24 dB: a soft voice at about 30 cm from a laptop mic, peaks
near -30 dBFS) with a faint deterministic noise floor (NOISE_DB, default
-62 dBFS) so the take is not digital silence between words. Stdlib only.

usage: python3 -I quiet_wav.py <in.wav> <out.wav> [gain_db] [noise_db]
"""
import array
import random
import sys
import wave


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    src, dst = sys.argv[1], sys.argv[2]
    gain_db = float(sys.argv[3]) if len(sys.argv) > 3 else -24.0
    noise_db = float(sys.argv[4]) if len(sys.argv) > 4 else -62.0
    with wave.open(src, "rb") as r:
        if (r.getnchannels(), r.getsampwidth(), r.getframerate()) != (1, 2, 16000):
            print("expected 16 kHz mono 16-bit PCM (say --data-format=LEI16@16000)", file=sys.stderr)
            return 2
        samples = array.array("h", r.readframes(r.getnframes()))
    if sys.byteorder != "little":
        samples.byteswap()
    gain = 10 ** (gain_db / 20)
    noise = 32767 * 10 ** (noise_db / 20)
    rng = random.Random(32)  # the same file every run
    out = array.array("h")
    peak = 0
    for s in samples:
        v = int(round(s * gain + rng.uniform(-noise, noise)))
        v = max(-32768, min(32767, v))
        peak = max(peak, abs(v))
        out.append(v)
    if sys.byteorder != "little":
        out.byteswap()
    with wave.open(dst, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(16000)
        w.writeframes(out.tobytes())
    import math

    peak_db = 20 * math.log10(peak / 32767) if peak else float("-inf")
    print(f"{dst}: {len(out) / 16000:.2f} s, gain {gain_db:+.0f} dB, peak {peak_db:.1f} dBFS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
