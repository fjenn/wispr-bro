// SPDX-License-Identifier: GPL-3.0-or-later
// OT2-H: read-only probe of Apple Dictation's shortcut (symbolic hotkey 164)
// in WindowServer's live table. Never changes anything.
//   clang scripts/wb-ot2h/probe.c -o <bin>/probe164
//   <bin>/probe164   # "164 live=1 armed=1" (fn twice fires) or "… armed=0"
// armed = the live value is still the fn modifier; that, not the enabled
// flag, is what every app's double-tap detection uses (see
// src-tauri/src/fork/shortcut/apple_dictation.rs).
// The preference side: defaults read com.apple.symbolichotkeys
// AppleSymbolicHotKeys | grep -A10 '164 ='; and
// defaults read com.apple.assistant.support "Dictation Enabled".
// Every app's reaction to a change: log stream --predicate
//   'eventMessage CONTAINS "AdjustToIronwood"'
#include <dlfcn.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
typedef bool (*IsEnabled)(int);
typedef int (*GetModifier)(int, uint32_t *, uint32_t *, uint32_t *);
int main(void) {
  void *sky = dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", RTLD_LAZY);
  IsEnabled CGSIsSymbolicHotKeyEnabled = sky ? (IsEnabled)dlsym(sky, "CGSIsSymbolicHotKeyEnabled") : 0;
  GetModifier CGSGetSymbolicHotModifierValue =
      sky ? (GetModifier)dlsym(sky, "CGSGetSymbolicHotModifierValue") : 0;
  if (!CGSIsSymbolicHotKeyEnabled || !CGSGetSymbolicHotModifierValue) {
    printf("164 unavailable\n");
    return 1;
  }
  uint32_t a = 0, flag = 0, mask = 0;
  int err = CGSGetSymbolicHotModifierValue(164, &a, &flag, &mask);
  printf("164 live=%d armed=%d\n", CGSIsSymbolicHotKeyEnabled(164) ? 1 : 0,
         (err == 0 && (flag & 0x800000)) ? 1 : 0);
  return 0;
}
