# SPDX-License-Identifier: GPL-3.0-or-later
"""Owner test 1: a café-like WAV for the meter check (16 kHz mono 16-bit):
4 s of pink-ish noise with a 120 Hz hum (about -30 dBFS), speech from
`say` mixed over the same noise, 3 s more noise, 2 s of digital silence.
Writes <wav> and <wav>.json with the segment times.

usage: make-wav.py <out.wav>
       SPEECH_PEAK=0.15 make-wav.py <out.wav>   (speech peak, default 0.6;
       Design W4's waveform check compares an ordinary and a loud take)
"""
import json
import math
import os
import random
import struct
import subprocess
import sys
import tempfile
import wave

SR = 16000
out = sys.argv[1]
os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
with tempfile.TemporaryDirectory() as tmp:
    speech_wav = os.path.join(tmp, "speech.wav")
    subprocess.run(
        ["say", "-o", speech_wav, "--file-format=WAVE", "--data-format=LEI16@16000",
         "Hi, I'm testing my microphone in a busy cafe. One, two, three, "
         "this should move the meter."],
        check=True,
    )
    w = wave.open(speech_wav)
    raw = w.readframes(w.getnframes())
    speech = [s / 32768 for s in struct.unpack("<%dh" % (len(raw) // 2), raw)]

random.seed(1)
state = [0.0, 0.0, 0.0]


def cafe(n):
    out = []
    for i in range(n):
        x = random.uniform(-1, 1)
        state[0] = 0.99765 * state[0] + x * 0.0990460
        state[1] = 0.96300 * state[1] + x * 0.2965164
        state[2] = 0.57000 * state[2] + x * 1.0526913
        pink = (state[0] + state[1] + state[2] + x * 0.1848) * 0.05
        out.append(pink * 0.35 + 0.006 * math.sin(2 * math.pi * 120 * i / SR))
    return out


gain = float(os.environ.get("SPEECH_PEAK", "0.6")) / max(abs(s) for s in speech)
pre = cafe(SR * 4)
voiced = [a + b * gain for a, b in zip(cafe(len(speech)), speech)]
post = cafe(SR * 3)
mix = pre + voiced + post + [0.0] * (SR * 2)
f = wave.open(out, "w")
f.setnchannels(1)
f.setsampwidth(2)
f.setframerate(SR)
f.writeframes(b"".join(struct.pack("<h", max(-32768, min(32767, int(x * 32767)))) for x in mix))
f.close()
t_speech = len(voiced) / SR
segments = {"noise": [0, 4], "speech": [4, 4 + t_speech],
            "noise_after": [4 + t_speech, 7 + t_speech],
            "silence": [7 + t_speech, 9 + t_speech]}
json.dump(segments, open(out + ".json", "w"), indent=1)
print(f"wrote {out}: {len(mix) / SR:.1f} s, segments {segments}")
