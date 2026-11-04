// SPDX-License-Identifier: GPL-3.0-or-later
// OT2-H: read-only probe of Apple Dictation's shortcut (symbolic hotkey 164)
// in WindowServer's live table. Never changes anything.
//   clang scripts/wb-ot2h/probe.c -o <bin>/probe164 \
//     -F/System/Library/PrivateFrameworks -framework SkyLight
//   <bin>/probe164   # "164 live=1" (on) or "164 live=0" (off)
// The preference side: defaults read com.apple.symbolichotkeys
// AppleSymbolicHotKeys | grep -A10 '164 ='; and
// defaults read com.apple.assistant.support "Dictation Enabled".
#include <stdbool.h>
#include <stdio.h>
extern bool CGSIsSymbolicHotKeyEnabled(int hotkey);
int main(void) {
  printf("164 live=%d\n", CGSIsSymbolicHotKeyEnabled(164) ? 1 : 0);
  return 0;
}
