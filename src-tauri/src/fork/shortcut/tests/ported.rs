// SPDX-License-Identifier: GPL-3.0-or-later
//! Upstream hotkey state-machine test vectors, ported to Wispr Bro's fn
//! grammar (R5 §S-01 rules 1-7, DEC-47, DEC-54) as [`Scenario`]s.
//!
//! Each upstream case is re-expressed under OUR rules and timings
//! (T = 500 ms, T_show = 150 ms). Where the upstream assertion differs from
//! our spec, the expected effects follow our spec and a `DIVERGES:` comment
//! says how. Modifier keys other than fn are not grammar inputs (the caller
//! drops them), so a Shift/Cmd/Opt/Ctrl event in an upstream case appears
//! only in the comment. Only the input sequences and expected outcomes are
//! taken from upstream; no upstream code is copied.
//!
//! Test vectors ported to Wispr Bro's grammar timings (T=500 ms,
//! T_show=150 ms); modified by Wispr Bro contributors, 2026-10-30.

use super::scenarios::Scenario;
use crate::fork::shortcut::processor::{CancelWhy, Effect, Input, Key, LockVia, Ms, StopVia};

fn d() -> Input {
    Input::FnDown
}
fn u() -> Input {
    Input::FnUp
}
fn key(name: &str) -> Input {
    Input::KeyDown(Key::Other(name.into()))
}
fn space() -> Input {
    Input::KeyDown(Key::Space)
}
fn esc() -> Input {
    Input::KeyDown(Key::Escape)
}
const OPEN: Effect = Effect::MicOpen;
const CLOSE: Effect = Effect::MicClose;
const START: Effect = Effect::TakeStarted;
const TAP: Effect = Effect::TapIgnored;
const RELEASE: Effect = Effect::Stopped {
    via: StopVia::Release,
};
const PRESS_STOP: Effect = Effect::Stopped {
    via: StopVia::LockedPress,
};
const DOUBLE: Effect = Effect::Locked {
    via: LockVia::DoubleTap,
};
const FNSPACE: Effect = Effect::Locked {
    via: LockVia::FnSpace,
};
const NO_SECOND: Effect = Effect::Cancelled {
    why: CancelWhy::NoSecondPress,
};
const ESC_CANCEL: Effect = Effect::Cancelled {
    why: CancelWhy::Escape,
};
/// A chord after the take ran ≥ T: dropped, transcript kept for History
/// (DEC-54).
fn chord_kept(k: &str) -> Effect {
    Effect::ChordIgnored {
        key: k.into(),
        mic_was_open: true,
        keep: true,
    }
}
fn chord(k: &str, mic_was_open: bool) -> Effect {
    Effect::ChordIgnored {
        key: k.into(),
        mic_was_open,
        keep: false,
    }
}
fn sc(
    name: &'static str,
    inputs: Vec<(Ms, Input)>,
    end: Ms,
    expect: Vec<(Ms, Effect)>,
) -> Scenario {
    Scenario {
        name,
        inputs,
        end,
        expect,
    }
}
/// A push-to-talk take held from `down` and released at `up` (≥ T).
fn ptt(down: Ms, up: Ms) -> Vec<(Ms, Effect)> {
    vec![
        (down + 150, OPEN),
        (down + 150, START),
        (up, RELEASE),
        (up, CLOSE),
    ]
}

pub fn ported_scenarios() -> Vec<Scenario> {
    let mut v = Vec::new();
    v.extend(anarlog());
    v.extend(macparakeet());
    v.extend(fluidvoice());
    v.extend(openwhispr());
    v.extend(hex());
    v.extend(hex_legacy());
    v
}

// ---------------------------------------------------------------------------
// anarlog: https://github.com/fastrepl/anarlog
// SHA a82755f4604fc30c9c89fe3e97f4195988d8ca46
// Files: crates/shortcut-macos/tests/processor.rs (cases),
//        crates/shortcut-macos/src/processor.rs, src/decision.rs (semantics:
//        DOUBLE_TAP_WINDOW 300 ms, MODIFIER_ONLY_MIN 300 ms, CANCEL_WINDOW 1 s)
// Licence: MIT. Copyright (c) 2023-present Fastrepl, Inc.
// Test vectors ported to Wispr Bro's grammar timings (T=500 ms,
// T_show=150 ms); modified by Wispr Bro contributors, 2026-10-30.
// Upstream hotkeys are Option / Cmd+A / Option+Cmd / Fn; every case is
// re-expressed with fn as the bare-modifier trigger.
// ---------------------------------------------------------------------------
fn anarlog() -> Vec<Scenario> {
    vec![
        // DIVERGES: upstream stops and inserts a 200 ms modifier-only hold; ours
        // treats a release before T as a tap and cancels with no second press.
        sc(
            "anarlog/press_and_hold_stops_on_release_modifier_only: 200 ms press is a tap",
            vec![(0, d()), (200, u())],
            1500,
            vec![(150, OPEN), (150, START), (700, NO_SECOND), (700, CLOSE)],
        ),
        // Upstream arms Option first, then Option+Cmd. Here Shift (dropped)
        // comes first and fn joins at 100 ms: the take is timed from fn alone.
        sc(
            "anarlog/press_and_hold_stops_on_release_multiple_modifiers: Shift then fn, timed from fn",
            vec![(100, d()), (1100, u())],
            2000,
            ptt(100, 1100),
        ),
        // DIVERGES: upstream stops and inserts on another key within 1 s
        // (CANCEL_WINDOW); ours drops the take (rule 2, DEC-54).
        sc(
            "anarlog/press_and_hold_cancels_on_other_key_standard: key at 500 ms drops the take",
            vec![(0, d()), (500, key("B")), (700, u())],
            1500,
            vec![
                (150, OPEN),
                (150, START),
                (500, chord_kept("B")),
                (500, CLOSE),
            ],
        ),
        // DIVERGES: upstream keeps recording when the other key comes after
        // CANCEL_WINDOW; DEC-54 drops the take at any time in a PTT press.
        sc(
            "anarlog/press_and_hold_does_not_cancel_after_threshold_standard: key at 1.5 s still drops (DEC-54)",
            vec![(0, d()), (1500, key("B")), (2000, u())],
            3000,
            vec![
                (150, OPEN),
                (150, START),
                (1500, chord_kept("B")),
                (1500, CLOSE),
            ],
        ),
        // Cmd added at 500 ms is not an input: the take ends on fn release.
        sc(
            "anarlog/press_and_hold_ignores_extra_modifier_after_threshold_modifier_only: Cmd ignored",
            vec![(0, d()), (800, u())],
            2000,
            ptt(0, 800),
        ),
        // DIVERGES: upstream discards on the extra modifier (Cmd at 100 ms);
        // ours ignores Cmd, so the take runs until the key C at 300 ms makes it
        // a chord. Fully releasing fn re-arms (dirty cleared) as upstream.
        sc(
            "anarlog/dirty_state_blocks_input_until_full_release: chord then fresh hold",
            vec![(0, d()), (300, key("C")), (400, u()), (500, d()), (1500, u())],
            2500,
            vec![
                (150, OPEN),
                (150, START),
                (300, chord("C", true)),
                (300, CLOSE),
                (650, OPEN),
                (650, START),
                (1500, RELEASE),
                (1500, CLOSE),
            ],
        ),
        // DIVERGES: upstream never activates on a backslide (Shift released
        // first). Shift is not an input for us, so fn 0-200 ms is a tap with a
        // take and the press at 300 ms is the second press: hands-free lock.
        sc(
            "anarlog/multiple_modifiers_no_backslide_activation: Shift backslide is a plain double tap",
            vec![(0, d()), (200, u()), (300, d()), (400, u())],
            1500,
            vec![(150, OPEN), (150, START), (300, DOUBLE)],
        ),
        // Upstream locks on the second release; ours on the second press.
        sc(
            "anarlog/double_tap_lock_modifier_only: lock on the second press",
            vec![(0, d()), (100, u()), (200, d()), (300, u())],
            1500,
            vec![(200, OPEN), (200, DOUBLE)],
        ),
        sc(
            "anarlog/double_tap_lock_stops_on_next_tap_modifier_only: press 800 ms after lock stops",
            vec![
                (0, d()),
                (100, u()),
                (200, d()),
                (300, u()),
                (1000, d()),
                (1100, u()),
            ],
            2000,
            vec![(200, OPEN), (200, DOUBLE), (1000, PRESS_STOP), (1000, CLOSE)],
        ),
        // DIVERGES: upstream window is 300 ms, so a second press 300 ms after
        // the release is "slow"; with our T = 500 ms it still locks.
        sc(
            "anarlog/double_tap_ignored_when_slow_standard: 300 ms gap locks at T=500",
            vec![(0, d()), (100, u()), (400, d()), (500, u())],
            1500,
            vec![(400, OPEN), (400, DOUBLE)],
        ),
        // The same case expressed at our window: 550 ms gap is slow, the
        // first tap is ignored and the second press is a fresh hold.
        sc(
            "anarlog/double_tap_ignored_when_slow_standard: 550 ms gap at T=500 is a fresh hold",
            vec![(0, d()), (100, u()), (650, d()), (1650, u())],
            2500,
            vec![
                (600, TAP),
                (800, OPEN),
                (800, START),
                (1650, RELEASE),
                (1650, CLOSE),
            ],
        ),
        sc(
            "anarlog/escape_cancels_from_hold: Esc at exactly T cancels",
            vec![(0, d()), (500, esc()), (700, u())],
            1500,
            vec![(150, OPEN), (150, START), (500, ESC_CANCEL), (500, CLOSE)],
        ),
        // After Esc with fn held, other keys pass through and nothing restarts
        // until fn is fully released and pressed again.
        sc(
            "anarlog/escape_while_holding_hotkey_does_not_restart: needs full release",
            vec![
                (0, d()),
                (500, esc()),
                (600, key("A")),
                (700, u()),
                (800, d()),
                (1800, u()),
            ],
            2500,
            vec![
                (150, OPEN),
                (150, START),
                (500, ESC_CANCEL),
                (500, CLOSE),
                (950, OPEN),
                (950, START),
                (1800, RELEASE),
                (1800, CLOSE),
            ],
        ),
        // DIVERGES: upstream stops when one of two hotkey modifiers is
        // released. With fn+Shift, releasing Shift is not an input; only the
        // fn release at 1.2 s stops.
        sc(
            "anarlog/multiple_modifiers_partial_release_stops: Shift release ignored, fn release stops",
            vec![(0, d()), (1200, u())],
            2000,
            ptt(0, 1200),
        ),
        // fn+T pressed together is a chord; the later 100 ms fn press is a
        // quick tap (upstream: start/stop recording; ours: ignored tap).
        sc(
            "anarlog/modifier_only_does_not_trigger_with_other_keys: fn+T then quick tap",
            vec![(0, d()), (0, key("T")), (100, u()), (200, d()), (300, u())],
            1500,
            vec![(0, chord("T", false)), (800, TAP)],
        ),
        // DIVERGES: upstream fn 200-600 ms stops and inserts; 400 ms < T is a
        // tap for us (take kept for a second press, then cancelled).
        sc(
            "anarlog/modifier_only_fn_triggers_after_fn_plus_key_then_full_release",
            vec![(0, d()), (0, key("C")), (50, u()), (200, d()), (600, u())],
            1500,
            vec![
                (0, chord("C", false)),
                (350, OPEN),
                (350, START),
                (1100, NO_SECOND),
                (1100, CLOSE),
            ],
        ),
        // The C key-up at 50 ms (fn still held) is not an input and does not
        // re-arm the press.
        sc(
            "anarlog/modifier_only_fn_does_not_trigger_when_fn_held_after_key_release",
            vec![(0, d()), (0, key("C")), (100, u()), (250, d()), (700, u())],
            1500,
            vec![
                (0, chord("C", false)),
                (400, OPEN),
                (400, START),
                (1200, NO_SECOND),
                (1200, CLOSE),
            ],
        ),
        sc(
            "anarlog/mouse_click_discards_modifier_only_within_threshold: click is a chord",
            vec![(0, d()), (100, key("MouseLeft")), (200, u())],
            1000,
            vec![(100, chord("MouseLeft", false))],
        ),
        // DIVERGES: upstream ignores a click after MODIFIER_ONLY_MIN; for us a
        // mouse button is a non-modifier key, so DEC-54 drops the take.
        sc(
            "anarlog/mouse_click_ignored_modifier_only_after_threshold: click after T_show drops (DEC-54)",
            vec![(0, d()), (500, key("MouseLeft")), (600, u())],
            1500,
            vec![
                (150, OPEN),
                (150, START),
                (500, chord_kept("MouseLeft")),
                (500, CLOSE),
            ],
        ),
    ]
}

// ---------------------------------------------------------------------------
// MacParakeet: https://github.com/moona3k/macparakeet
// SHA a388b7d449c722875f24e26df35acecf881dd35e
// Files: Tests/MacParakeetTests/Hotkey/FnKeyStateMachineTests.swift,
//        Tests/MacParakeetTests/Hotkey/HotkeyGestureControllerTests.swift
//        (semantics from Sources/MacParakeetCore/STT/FnKeyStateMachine.swift:
//        tap threshold 400 ms, startup debounce 100 ms, cancel window 5 s)
// Licence: GPL-3.0. Copyright (C) 2026 Daniel Moon
// Test vectors ported to Wispr Bro's grammar timings (T=500 ms,
// T_show=150 ms); modified by Wispr Bro contributors, 2026-10-30.
// Upstream "startup timer" = our T_show, "hold timer" = our T.
// ---------------------------------------------------------------------------
fn macparakeet() -> Vec<Scenario> {
    vec![
        sc(
            "macparakeet/FnKeyStateMachineTests.testDoubleTapStartsPersistentRecording",
            vec![(1000, d()), (1050, u()), (1200, d()), (1250, u())],
            3000,
            vec![(1200, OPEN), (1200, DOUBLE)],
        ),
        sc(
            "macparakeet/FnKeyStateMachineTests.testPersistentStopsOnThirdFnPress",
            vec![
                (1000, d()),
                (1050, u()),
                (1200, d()),
                (1250, u()),
                (2000, d()),
                (2050, u()),
            ],
            3000,
            vec![(1200, OPEN), (1200, DOUBLE), (2000, PRESS_STOP), (2000, CLOSE)],
        ),
        // fn held and never released within the run: one take, still open.
        sc(
            "macparakeet/FnKeyStateMachineTests.testHoldTimerStartsHoldToTalk: take open at T_show",
            vec![(1000, d())],
            3000,
            vec![(1150, OPEN), (1150, START)],
        ),
        sc(
            "macparakeet/FnKeyStateMachineTests.testHoldReleaseStopsRecording: 500 ms hold inserts",
            vec![(1000, d()), (1500, u())],
            3000,
            ptt(1000, 1500),
        ),
        // DIVERGES: 450 ms gap is past upstream's 400 ms window (new first
        // tap); within our T it locks.
        sc(
            "macparakeet/FnKeyStateMachineTests.testSlowTapsDoNotDoubleTap: 450 ms gap locks at T=500",
            vec![(1000, d()), (1050, u()), (1500, d()), (1550, u())],
            3000,
            vec![(1500, OPEN), (1500, DOUBLE)],
        ),
        sc(
            "macparakeet/FnKeyStateMachineTests.testCustomTapThresholdAllowsLongerDoubleTapWindow: 440 ms gap",
            vec![(1000, d()), (1050, u()), (1490, d()), (1540, u())],
            3000,
            vec![(1490, OPEN), (1490, DOUBLE)],
        ),
        // Esc with the locking press still held: cancel; the release is inert.
        sc(
            "macparakeet/FnKeyStateMachineTests.testEscapeDuringPersistentCancels: fn still held",
            vec![(1000, d()), (1050, u()), (1200, d()), (1300, esc()), (1400, u())],
            3000,
            vec![(1200, OPEN), (1200, DOUBLE), (1300, ESC_CANCEL), (1300, CLOSE)],
        ),
        sc(
            "macparakeet/FnKeyStateMachineTests.testEscapeDuringHoldCancels: Esc after T",
            vec![(1000, d()), (1600, esc()), (1700, u())],
            3000,
            vec![(1150, OPEN), (1150, START), (1600, ESC_CANCEL), (1600, CLOSE)],
        ),
        sc(
            "macparakeet/FnKeyStateMachineTests.testEscapeDuringProvisionalRecordingCancels: Esc before T",
            vec![(1000, d()), (1200, esc()), (1300, u())],
            3000,
            vec![(1150, OPEN), (1150, START), (1200, ESC_CANCEL), (1200, CLOSE)],
        ),
        sc(
            "macparakeet/FnKeyStateMachineTests.testEscapeAfterQuickReleaseDoesNothing",
            vec![(1000, d()), (1050, u()), (1100, esc())],
            3000,
            vec![(1550, TAP)],
        ),
        // DIVERGES: upstream blocks fn for a 5 s undo window after a cancel;
        // ours has no cancel window, so the next press is a fresh quick tap.
        sc(
            "macparakeet/FnKeyStateMachineTests.testFnBlockedDuringCancelWindow: no cancel window",
            vec![
                (1000, d()),
                (1050, u()),
                (1200, d()),
                (1300, esc()),
                (1350, u()),
                (2000, d()),
                (2100, u()),
            ],
            3000,
            vec![
                (1200, OPEN),
                (1200, DOUBLE),
                (1300, ESC_CANCEL),
                (1300, CLOSE),
                (2600, TAP),
            ],
        ),
        sc(
            "macparakeet/FnKeyStateMachineTests.testFnUpInIdleState: stray release",
            vec![(1000, u())],
            2000,
            vec![],
        ),
        // Upstream discards and shows the "ready" pill; ours keeps the take for
        // a possible second press and cancels when T passes.
        sc(
            "macparakeet/FnKeyStateMachineTests.testQuickReleaseDiscardsProvisionalRecording",
            vec![(1000, d()), (1200, u())],
            3000,
            vec![(1150, OPEN), (1150, START), (1700, NO_SECOND), (1700, CLOSE)],
        ),
        // DIVERGES: 450 ms ≥ upstream's 400 ms threshold stops and inserts;
        // under our T it is a tap.
        sc(
            "macparakeet/FnKeyStateMachineTests.testHeldReleaseStopsAfterDebouncedStartEvenIfHoldTimerDidNotFire",
            vec![(1000, d()), (1450, u())],
            3000,
            vec![(1150, OPEN), (1150, START), (1950, NO_SECOND), (1950, CLOSE)],
        ),
        sc(
            "macparakeet/FnKeyStateMachineTests.testInterruptBeforeStartupTimerDoesNotDiscard",
            vec![(1000, d()), (1050, key("RightArrow")), (1100, u())],
            3000,
            vec![(1050, chord("RightArrow", false))],
        ),
        sc(
            "macparakeet/FnKeyStateMachineTests.testInterruptAfterStartupTimerDiscardsSilently",
            vec![(1000, d()), (1200, key("RightArrow")), (1300, u())],
            3000,
            vec![
                (1150, OPEN),
                (1150, START),
                (1200, chord("RightArrow", true)),
                (1200, CLOSE),
            ],
        ),
        // Upstream cancels a confirmed hold on interruption and the release
        // then does nothing; ours matches (DEC-54).
        sc(
            "macparakeet/HotkeyGestureControllerTests.testInterruptionDuringConfirmedHoldCancelsRecordingImmediately",
            vec![(1000, d()), (1600, key("DownArrow")), (1700, u())],
            3000,
            vec![
                (1150, OPEN),
                (1150, START),
                (1600, chord_kept("DownArrow")),
                (1600, CLOSE),
            ],
        ),
        // DIVERGES: upstream Esc in the ready-for-second-tap window resets the
        // gesture. With no take active Esc is not ours (rule 7), so the
        // second press still locks.
        sc(
            "macparakeet/HotkeyGestureControllerTests.testEscapeDuringReadyWindowResetsWithoutShowingIdleEscape",
            vec![(1000, d()), (1050, u()), (1100, esc()), (1200, d()), (1250, u())],
            3000,
            vec![(1200, OPEN), (1200, DOUBLE)],
        ),
        // DIVERGES: upstream cancels when the trigger is released together
        // with another modifier. Modifiers are not inputs, so a release after
        // T stops and inserts.
        sc(
            "macparakeet/HotkeyGestureControllerTests.testNonBareReleaseDuringHoldCancelsRecording: plain stop",
            vec![(1000, d()), (1600, u())],
            3000,
            ptt(1000, 1600),
        ),
        // Esc while pending (no take) is a chord; holding on past T_show and T
        // never starts a take.
        sc(
            "macparakeet/HotkeyGestureControllerTests.testEscapeWithoutOwnershipClearsPendingFirstPress",
            vec![(1000, d()), (1100, esc()), (2000, u())],
            3000,
            vec![(1100, chord("Escape", false))],
        ),
    ]
}

// ---------------------------------------------------------------------------
// FluidVoice: https://github.com/altic-dev/FluidVoice
// SHA ea94c8a6736d453c764a2431503832b3fb134911
// File: Tests/FluidDictationIntegrationTests/HotkeyShortcutTests.swift
//       (ModifierOnlyFlagsReplay / ModifierOnlyShortcutFlagsDecision cases
//       and the GlobalHotkeyManager key-sequence cases)
// Licence: GPL-3.0. Copyright: no copyright line in LICENSE or the test file;
//       credit "FluidVoice contributors (altic-dev)".
// Test vectors ported to Wispr Bro's grammar timings (T=500 ms,
// T_show=150 ms); modified by Wispr Bro contributors, 2026-10-30.
// Upstream cases are untimed; times below are chosen. Upstream's "clean tap
// starts recording" (toggle mode) maps to our double-tap lock or quick tap.
// ---------------------------------------------------------------------------
fn fluidvoice() -> Vec<Scenario> {
    vec![
        // Owner fn (63) with each extra modifier family/side and both release
        // orders: all 16 upstream variants are the same input for us (the
        // extra modifier is dropped). Typing makes a chord; the next clean
        // double tap still locks.
        sc(
            "fluidvoice/testModifierOnlyInterruptionsAcrossFamiliesSidesAndReleaseOrders: fn owner, next double tap works",
            vec![
                (0, d()),
                (80, key("Return")),
                (120, u()),
                (400, d()),
                (460, u()),
                (600, d()),
                (660, u()),
            ],
            2000,
            vec![(80, chord("Return", false)), (600, OPEN), (600, DOUBLE)],
        ),
        // fn+click must not read as a tap (no TapIgnored, no tap window).
        sc(
            "fluidvoice/testModifierOnlyShortcutIgnoresTapAfterMouseClick: fn+click is no tap",
            vec![(0, d()), (60, key("MouseLeft")), (100, u())],
            1000,
            vec![(60, chord("MouseLeft", false))],
        ),
        // Shift (dropped) + Return during the press: chord; Shift's release
        // cannot re-arm the press.
        sc(
            "fluidvoice/testModifierOnlyShortcutDoesNotFireOnUnrelatedShiftKeyCombo: Return after T_show",
            vec![(0, d()), (400, key("Return")), (450, u())],
            1500,
            vec![
                (150, OPEN),
                (150, START),
                (400, chord("Return", true)),
                (400, CLOSE),
            ],
        ),
        // Sibling-side press while armed: arrives as a duplicate fn down and
        // must not erase the interruption.
        sc(
            "fluidvoice/testBranch2ModifierOnlyShortcutSiblingPressDoesNotEraseInterrupt: duplicate fn down",
            vec![(0, d()), (60, key("A")), (100, d()), (300, u())],
            1500,
            vec![(60, chord("A", false))],
        ),
        // DIVERGES: upstream refuses to arm fn-only when Cmd is already held;
        // the caller drops Cmd, so fn with Cmd held dictates normally.
        sc(
            "fluidvoice/testExplicitModifierShortcutRejectsUntrackedExtraFlags: Cmd held, fn still dictates",
            vec![(0, d()), (700, u())],
            1500,
            ptt(0, 700),
        ),
        sc(
            "fluidvoice/testRealManagerModifierTypingDoesNotStartAndNextCleanTapWorks: Return, R, then tap",
            vec![
                (0, d()),
                (200, key("Return")),
                (250, key("R")),
                (300, u()),
                (1000, d()),
                (1060, u()),
            ],
            2000,
            vec![
                (150, OPEN),
                (150, START),
                (200, chord("Return", true)),
                (200, CLOSE),
                (1560, TAP),
            ],
        ),
        // Plain O/Return/keypad Enter never start; fn+Delete is a chord.
        sc(
            "fluidvoice/testIssues1031And909UnconfiguredOAndReturnPassThrough: fn+Delete is a chord",
            vec![
                (0, key("O")),
                (10, key("Return")),
                (20, key("KeypadEnter")),
                (100, d()),
                (130, key("Delete")),
                (180, u()),
            ],
            1000,
            vec![(130, chord("Delete", false))],
        ),
        // Cmd+C/V/Z/Tab/W/Y without fn: nothing; fn still works afterwards.
        sc(
            "fluidvoice/testIssues675And609OrdinaryCommandChordsDoNotTriggerOtherModifiers",
            vec![
                (0, key("C")),
                (50, key("V")),
                (100, key("Z")),
                (150, key("Tab")),
                (200, key("W")),
                (250, key("Y")),
                (1000, d()),
                (1800, u()),
            ],
            2500,
            ptt(1000, 1800),
        ),
        // Autorepeat analogue: repeated fn downs while held never restart.
        sc(
            "fluidvoice/testOtherRecordingModesIgnoreAutorepeat: repeated fn downs, one take",
            (0..18)
                .map(|i| (i * 50, d()))
                .chain([(900, u())])
                .collect(),
            2000,
            ptt(0, 900),
        ),
        // DIVERGES: upstream toggle mode starts then stops once on a rapid
        // double tap; ours locks (hands-free) and nothing stops it.
        sc(
            "fluidvoice/testPrimaryToggleRapidDoubleTapStartsThenStopsOnce: 10 ms taps lock",
            vec![(0, d()), (10, u()), (20, d()), (30, u())],
            2000,
            vec![(20, OPEN), (20, DOUBLE)],
        ),
        // fn+Ctrl [63, 59], Ctrl pressed first and released last: Ctrl is not
        // an input (Command Mode off), so the take runs on fn alone.
        sc(
            "fluidvoice/testIssues221And327MultiModifierChordsWorkInEveryPressAndReleaseOrder: fn+Ctrl",
            vec![(50, d()), (900, u())],
            2000,
            ptt(50, 900),
        ),
    ]
}

// ---------------------------------------------------------------------------
// OpenWhispr: https://github.com/OpenWhispr/openwhispr
// SHA b9e9ebefac4b961456c269164a93ddfdd53438b9
// Files: resources/macos-globe-listener.swift (FN_INTERRUPTED emitted once per
//        fn press on the first keyDown while fn is down), main.js
//        ("globe-interrupted" handler; MIN_HOLD_DURATION_MS 150,
//        POST_STOP_COOLDOWN_MS 300)
// Licence: MIT. Copyright (c) 2024 OpenWhispr Team
// Test vectors ported to Wispr Bro's grammar timings (T=500 ms,
// T_show=150 ms); modified by Wispr Bro contributors, 2026-10-30.
// The rule has no upstream tests; these vectors are derived from its code.
// ---------------------------------------------------------------------------
fn openwhispr() -> Vec<Scenario> {
    vec![
        sc(
            "openwhispr/FN_INTERRUPTED: key 1 ms before T_show cancels preparation, mic never opens",
            vec![(0, d()), (149, key("RightArrow")), (200, u())],
            1000,
            vec![(149, chord("RightArrow", false))],
        ),
        sc(
            "openwhispr/FN_INTERRUPTED: key exactly at T_show comes after the take started",
            vec![(0, d()), (150, key("RightArrow")), (200, u())],
            1000,
            vec![
                (150, OPEN),
                (150, START),
                (150, chord("RightArrow", true)),
                (150, CLOSE),
            ],
        ),
        sc(
            "openwhispr/FN_INTERRUPTED: key 10 s into a hold cancels the recording",
            vec![(0, d()), (10_000, key("LeftArrow")), (10_500, u())],
            11_000,
            vec![
                (150, OPEN),
                (150, START),
                (10_000, chord_kept("LeftArrow")),
                (10_000, CLOSE),
            ],
        ),
        // Latched once per press; the release after T does not insert.
        sc(
            "openwhispr/FN_INTERRUPTED: latched once per press, release after T inserts nothing",
            vec![
                (0, d()),
                (200, key("UpArrow")),
                (300, key("UpArrow")),
                (400, key("DownArrow")),
                (600, u()),
            ],
            1500,
            vec![
                (150, OPEN),
                (150, START),
                (200, chord("UpArrow", true)),
                (200, CLOSE),
            ],
        ),
        // The latch resets on the next fn press.
        sc(
            "openwhispr/FN_INTERRUPTED: latch resets on the next fn press",
            vec![
                (0, d()),
                (50, key("LeftArrow")),
                (100, u()),
                (1000, d()),
                (1300, key("RightArrow")),
                (1400, u()),
            ],
            2000,
            vec![
                (50, chord("LeftArrow", false)),
                (1150, OPEN),
                (1150, START),
                (1300, chord("RightArrow", true)),
                (1300, CLOSE),
            ],
        ),
        // Upstream ignores the interrupt outside a bare-fn push-to-talk press;
        // ours: keys while locked pass through even with fn held.
        sc(
            "openwhispr/FN_INTERRUPTED: ignored while locked with the locking press held",
            vec![
                (0, d()),
                (50, u()),
                (150, d()),
                (400, key("RightArrow")),
                (600, u()),
                (2000, d()),
                (2050, u()),
            ],
            3000,
            vec![(150, OPEN), (150, DOUBLE), (2000, PRESS_STOP), (2000, CLOSE)],
        ),
        // DIVERGES: upstream's keyDown monitor treats Space like any key and
        // cancels; rule 6 makes fn+Space a lock (Space swallowed).
        sc(
            "openwhispr/FN_INTERRUPTED: Space exactly at T_show locks instead (rule 6)",
            vec![(0, d()), (150, space()), (400, u())],
            1000,
            vec![(150, OPEN), (150, START), (150, FNSPACE)],
        ),
        // DIVERGES: upstream ignores a press within 300 ms of a stop; we have
        // no cooldown, so a press 100 ms after a stop is a new take.
        sc(
            "openwhispr/POST_STOP_COOLDOWN: no cooldown, press 100 ms after a stop is a new take",
            vec![(0, d()), (800, u()), (900, d()), (1900, u())],
            2500,
            vec![
                (150, OPEN),
                (150, START),
                (800, RELEASE),
                (800, CLOSE),
                (1050, OPEN),
                (1050, START),
                (1900, RELEASE),
                (1900, CLOSE),
            ],
        ),
    ]
}

// ---------------------------------------------------------------------------
// hex: https://github.com/anomalyco/hex
// SHA ed8c18e9a172a8a781a6999683085622897f0e83
// Files: src/keyboard.rs (post_chord_breaker: a marked synthetic key posted
//        once when a held press passes the hold threshold, so other apps read
//        the hold as a chord; the marked key never reaches the shortcut
//        machine), and the chord tests in src/suppression.rs
//        (a_held_modifier_only_shortcut_breaks_the_chord_once_after_the_hold_threshold,
//        early_chord_discards_until_everything_is_released,
//        hid_evidence_discards_a_chord_hidden_by_a_global_shortcut,
//        hid_evidence_and_visible_chord_keys_discard_once_in_either_release_order,
//        hidden_chords_in_quick_succession_never_double_tap_lock)
// Licence: MIT. Copyright (c) 2026 Christopher Langton
// Test vectors ported to Wispr Bro's grammar timings (T=500 ms,
// T_show=150 ms); modified by Wispr Bro contributors, 2026-10-30.
// Upstream hotkey is Option; HID key evidence and visible keys both arrive
// as KeyDown inputs for us.
// ---------------------------------------------------------------------------
fn hex() -> Vec<Scenario> {
    let mut held_twice = ptt(0, 3000);
    held_twice.extend(ptt(5000, 8000));
    vec![
        // The chord breaker is our own marked key: not an input, so a long
        // hold is not dropped as a chord. Two presses as upstream.
        sc(
            "hex/keyboard.rs post_chord_breaker: own marked key never drops a held take",
            vec![(0, d()), (3000, u()), (5000, d()), (8000, u())],
            9000,
            held_twice,
        ),
        // Key released after fn: the fresh press right after is a hold.
        sc(
            "hex/early_chord_discards_until_everything_is_released: then a fresh hold",
            vec![
                (0, d()),
                (100, key("A")),
                (150, u()),
                (300, d()),
                (1300, u()),
            ],
            2000,
            vec![
                (100, chord("A", false)),
                (450, OPEN),
                (450, START),
                (1300, RELEASE),
                (1300, CLOSE),
            ],
        ),
        // The discarded chord is not the first tap of a double tap.
        sc(
            "hex/hid_evidence_discards_a_chord_hidden_by_a_global_shortcut: key 1 at 20 ms",
            vec![
                (0, d()),
                (20, key("1")),
                (70, u()),
                (1500, u()),
                (1600, d()),
                (2600, u()),
            ],
            3500,
            vec![
                (20, chord("1", false)),
                (1750, OPEN),
                (1750, START),
                (2600, RELEASE),
                (2600, CLOSE),
            ],
        ),
        sc(
            "hex/hid_evidence_discards_a_chord_hidden_by_a_global_shortcut: key 3 at 200 ms",
            vec![
                (0, d()),
                (200, key("3")),
                (250, u()),
                (1500, u()),
                (1600, d()),
                (2600, u()),
            ],
            3500,
            vec![
                (150, OPEN),
                (150, START),
                (200, chord("3", true)),
                (200, CLOSE),
                (1750, OPEN),
                (1750, START),
                (2600, RELEASE),
                (2600, CLOSE),
            ],
        ),
        // DIVERGES: upstream key 49 (Space) at 150 ms discards as a chord;
        // rule 6 makes fn+Space a lock, so the press at 1.6 s stops it.
        sc(
            "hex/hid_evidence_discards_a_chord_hidden_by_a_global_shortcut: Space at 150 ms locks",
            vec![
                (0, d()),
                (150, space()),
                (200, u()),
                (1500, u()),
                (1600, d()),
                (1700, u()),
            ],
            2500,
            vec![
                (150, OPEN),
                (150, START),
                (150, FNSPACE),
                (1600, PRESS_STOP),
                (1600, CLOSE),
            ],
        ),
        // The HID copy and the visible copy of one key discard once.
        sc(
            "hex/hid_evidence_and_visible_chord_keys_discard_once_in_either_release_order",
            vec![
                (0, d()),
                (60, key("1")),
                (60, key("1")),
                (140, u()),
                (1000, d()),
                (2000, u()),
            ],
            2500,
            vec![
                (60, chord("1", false)),
                (1150, OPEN),
                (1150, START),
                (2000, RELEASE),
                (2000, CLOSE),
            ],
        ),
        // Two quick fn+1 / fn+2 switches must never look like a double tap.
        sc(
            "hex/hidden_chords_in_quick_succession_never_double_tap_lock",
            vec![
                (0, d()),
                (40, key("1")),
                (160, u()),
                (280, d()),
                (310, key("2")),
                (400, u()),
            ],
            1500,
            vec![(40, chord("1", false)), (310, chord("2", false))],
        ),
    ]
}

// ---------------------------------------------------------------------------
// Hex (legacy): https://github.com/kitlangton/Hex
// SHA 3e300721b8784ea19ede0359d8fbcca94cfb6a04
// File: HexCore/Tests/HexCoreTests/HotKeyProcessorTests.swift
// Licence: MIT. Copyright (c) 2025 Kit Langton
// Test vectors ported to Wispr Bro's grammar timings (T=500 ms,
// T_show=150 ms); modified by Wispr Bro contributors, 2026-10-30.
// Credit only: anarlog's processor and tests derive from this file, so only
// the case with no anarlog counterpart is ported.
// ---------------------------------------------------------------------------
fn hex_legacy() -> Vec<Scenario> {
    vec![
        // Cmd pressed at 2 s and released at 2.1 s is not an input.
        sc(
            "hexlegacy/pressAndHold_staysDirtyAfterTwoSeconds: Cmd during a hold ignored",
            vec![(0, d()), (2200, u())],
            3000,
            ptt(0, 2200),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::scenarios::{run_scenario, scenarios};
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_ported_scenario_passes() {
        let all = ported_scenarios();
        let mut failures = Vec::new();
        for s in &all {
            if let Err(e) = run_scenario(s) {
                failures.push(format!("{}: {e}", s.name));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        assert!(all.len() >= 60, "only {} ported vectors", all.len());
    }

    #[test]
    fn ported_names_are_tagged_and_unique() {
        let tags = [
            "anarlog/",
            "macparakeet/",
            "fluidvoice/",
            "openwhispr/",
            "hex/",
            "hexlegacy/",
        ];
        let mut names = HashSet::new();
        for s in ported_scenarios() {
            assert!(tags.iter().any(|t| s.name.starts_with(t)), "{}", s.name);
            assert!(names.insert(s.name), "duplicate name {}", s.name);
        }
    }

    #[test]
    fn no_ported_input_sequence_duplicates_another() {
        let fmt = |s: &Scenario| format!("{:?}", s.inputs);
        let existing: HashSet<String> = scenarios().iter().map(fmt).collect();
        let mut seen = HashSet::new();
        for s in ported_scenarios() {
            let k = fmt(&s);
            assert!(
                !existing.contains(&k),
                "{} repeats a scripted scenario",
                s.name
            );
            assert!(seen.insert(k), "{} repeats another ported vector", s.name);
        }
    }
}
