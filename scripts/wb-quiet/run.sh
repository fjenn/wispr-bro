#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# OT2-B (MAC-TEST-SCRIPT "Owner test 2: quiet speech and whispers"), for the
# orchestrator's Mac wave. Quiet speech and whispers over café noise, from
# WAVs only: the app's test microphone plays them (WISPRBRO_TEST_MIC_WAV),
# never the speakers, never the real microphone, never the owner's app.
#
# usage: OUT=<dir> scripts/wb-quiet/run.sh prep
#          the WAVs: `say` sentences (make-speech.sh), then the quiet set's
#          takes (quiet_bench::write_wavs): normal-20dB, quiet-20-3dB,
#          whisper-30-3dB, whisper-35-0dB, each <name>.wav + .wav.json
#          (segments) + .txt (words), in <OUT>/wavs
#        EXE=<debug wispr-bro> OUT=<dir> scripts/wb-quiet/run.sh meter
#          OT2-B.1: each WAV through the mic test (scripts/wb-meter/run.sh,
#          QUIET=1 for the quiet ones), PASS/FAIL per segment
#        PID=<test app pid> DATA=<its Data dir> OUT=<dir> scripts/wb-quiet/run.sh takes
#          OT2-B.2/OT2-B.3: one take per WAV by SIGUSR2 into our own TextEdit
#          (start the app first with `scripts/wb-s05/app.sh start`); per take
#          the History text against the words (WER) and the bar's loudest
#          level per second from the debug log (`pill: level frames …`)
# Each take holds ~/wb-bench/gui.lock.
set -u
: "${OUT:?}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
. "$HERE/../wb-lib/app.sh"
WAVS="$OUT/wavs"
LOCK="$HOME/wb-bench/gui.lock"
NAMES="normal-20dB quiet-20-3dB whisper-30-3dB whisper-35-0dB"

prep() {
  "$HERE/make-speech.sh" "$OUT/speech" || return 1
  (cd "$ROOT/src-tauri" && WB_QUIET_SPEECH="$OUT/speech" WB_QUIET_OUT="$WAVS" \
    cargo test --lib fork::audio::quiet_bench::write_wavs -- --ignored --nocapture) | grep '^wrote'
}

meter() {
  : "${EXE:?}"
  local n ok=0
  for n in $NAMES; do
    local quiet=1
    [ "$n" = normal-20dB ] && quiet=
    if QUIET="$quiet" OUT="$OUT/meter-$n" "$ROOT/scripts/wb-meter/run.sh" "$WAVS/$n.wav"; then
      echo "OT2-B.1 $n: PASS"
    else
      echo "OT2-B.1 $n: FAIL"
      ok=1
    fi
  done
  return $ok
}

take_one() {
  local n="$1" before
  before="$(wc -l < "$DATA"/logs/*.log 2>/dev/null | tail -1)"
  wb_mic_load "$WAVS/$n.wav" || return 4
  # Only into our own TextEdit: if anything else is in front at either
  # end of the take, it is cancelled (nothing reaches another window).
  if [ "$(wb_front_pid)" != "${TE_PID:?}" ]; then
    echo "SKIP: our TextEdit ($TE_PID) is not in front"; return 5
  fi
  kill -USR2 "$PID"; sleep 0.6
  wb_mic_play "$WAVS/$n.wav"; sleep 0.4
  if [ "$(wb_front_pid)" != "$TE_PID" ]; then
    wb_cancel "$(lsof -a -p "$PID" -d cwd -Fn | sed -n 's/^n//p')" >/dev/null 2>&1
    echo "SKIP: focus moved during the take; cancelled"; return 5
  fi
  kill -USR2 "$PID"; sleep 5
  local text
  text="$(sqlite3 "$DATA/history.db" "SELECT transcription_text FROM transcription_history ORDER BY id DESC LIMIT 1")"
  python3 -I - "$WAVS/$n.txt" "$text" <<'EOF'
import re, sys
ref = open(sys.argv[1]).read()
hyp = sys.argv[2]
w = lambda s: [x for x in re.sub(r"[^a-z0-9' ]", " ", s.lower()).split() if x]
r, h = w(ref), w(hyp)
d = list(range(len(h) + 1))
for i, rw in enumerate(r):
    prev, d[0] = d[0], i + 1
    for j, hw in enumerate(h):
        cur = d[j + 1]
        d[j + 1] = prev if rw == hw else 1 + min(prev, d[j], d[j + 1])
        prev = cur
print(f"WER {100 * d[len(h)] / max(1, len(r)):.0f}% text {hyp!r}")
EOF
  # The bar: the loudest bar per second while the take ran.
  cat "$DATA"/logs/*.log 2>/dev/null | tail -n "+$((before + 1))" | grep -o "loudest bar [0-9.]*" | tr '\n' ' '
  echo
}

takes() {
  : "${PID:?}" "${DATA:?}"
  wb_test_mic_env "$DATA" || exit 1
  local te n
  : > "$OUT/take.txt"
  te="$(wb_textedit_open "$OUT/take.txt")" || exit 1
  sleep 2
  for n in $NAMES; do
    echo "== $n"
    TE_PID="$te" OT2_TAKE=1 /usr/bin/lockf -k -t 600 "$LOCK" "$0" take "$n"
  done
  wb_textedit_close "$te"
}

case "${1:-}" in
  prep) prep ;;
  meter) meter ;;
  takes) takes ;;
  take) [ "${OT2_TAKE:-}" = 1 ] && take_one "$2" ;;
  *) sed -n '2,24p' "$0"; exit 2 ;;
esac
