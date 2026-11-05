// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) the Tambourine authors (https://github.com/kstonekuan/tambourine-voice)
// Modified by Wispr Bro contributors, 2026-10-31.
//
// Ported from Tambourine `app/src-tauri/src/tests/audio_mute_tests.rs` and
// the tests at the end of `audio_mute/macos.rs` (commit
// c54b527d5920b821e1f0ff837457d69405696b6b, AGPL-3.0); see `mute.rs`. Ours:
// the per-device fake and the tests after "Wispr Bro" below.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::pure::{
    evaluate_volume_zero_fallback_mute_attempt, select_mute_strategy, MuteStrategy,
    VolumeZeroFallbackMuteAttemptResult,
};
use super::{
    ActiveMuteSession, AudioControlError, AudioMuteManager, MuteState, OutputVolumeScalarSnapshot,
    SystemAudioControl,
};

#[derive(Debug, Default)]
struct FakeAudioControllerState {
    is_muted: bool,
    is_muted_error: Option<String>,
    begin_mute_error: Option<String>,
    begin_mute_error_with_recovery_session: Option<String>,
    end_mute_error: Option<String>,
    begin_mute_call_count: usize,
    end_mute_call_count: usize,
}

#[derive(Clone)]
struct FakeAudioController {
    state: Arc<Mutex<FakeAudioControllerState>>,
}

impl FakeAudioController {
    fn new(state: Arc<Mutex<FakeAudioControllerState>>) -> Self {
        Self { state }
    }
}

fn fake_active_mute_session() -> ActiveMuteSession {
    ActiveMuteSession::MacOsDeviceMute {
        output_device_id: 42,
        initial_device_muted: false,
    }
}

fn is_expected_fake_session(active_mute_session: &ActiveMuteSession) -> bool {
    matches!(
        active_mute_session,
        ActiveMuteSession::MacOsDeviceMute { .. }
    )
}

impl SystemAudioControl for FakeAudioController {
    fn is_muted(&self) -> Result<bool, AudioControlError> {
        let state = self.state.lock().unwrap();
        if let Some(error_message) = &state.is_muted_error {
            return Err(AudioControlError::GetPropertyFailed(error_message.clone()));
        }

        Ok(state.is_muted)
    }

    fn begin_mute_session(&self) -> Result<ActiveMuteSession, AudioControlError> {
        let mut state = self.state.lock().unwrap();
        state.begin_mute_call_count += 1;

        if let Some(error_message) = &state.begin_mute_error {
            return Err(AudioControlError::SetPropertyFailed(error_message.clone()));
        }

        if let Some(error_message) = state.begin_mute_error_with_recovery_session.clone() {
            state.is_muted = true;
            return Err(AudioControlError::MuteSessionStartFailed {
                message: error_message,
                recovery_session: Some(fake_active_mute_session()),
            });
        }

        state.is_muted = true;
        Ok(fake_active_mute_session())
    }

    fn end_mute_session(
        &self,
        active_mute_session: &ActiveMuteSession,
    ) -> Result<(), AudioControlError> {
        let mut state = self.state.lock().unwrap();
        state.end_mute_call_count += 1;

        if !is_expected_fake_session(active_mute_session) {
            return Err(AudioControlError::SetPropertyFailed(
                "unexpected fake mute session type".to_string(),
            ));
        }

        if let Some(error_message) = &state.end_mute_error {
            return Err(AudioControlError::SetPropertyFailed(error_message.clone()));
        }

        state.is_muted = false;
        Ok(())
    }
}

fn manager_with(
    state: FakeAudioControllerState,
) -> (Arc<Mutex<FakeAudioControllerState>>, AudioMuteManager) {
    let state = Arc::new(Mutex::new(state));
    let manager =
        AudioMuteManager::from_controller(Box::new(FakeAudioController::new(state.clone())));
    (state, manager)
}

#[test]
fn mute_and_unmute_perform_expected_session_calls() {
    let (state, audio_mute_manager) = manager_with(FakeAudioControllerState::default());

    audio_mute_manager.mute().unwrap();
    audio_mute_manager.unmute().unwrap();

    let state_after_operations = state.lock().unwrap();
    assert_eq!(state_after_operations.begin_mute_call_count, 1);
    assert_eq!(state_after_operations.end_mute_call_count, 1);
    assert!(!state_after_operations.is_muted);
}

#[test]
fn mute_is_idempotent_when_already_muted_by_manager() {
    let (state, audio_mute_manager) = manager_with(FakeAudioControllerState::default());

    audio_mute_manager.mute().unwrap();
    audio_mute_manager.mute().unwrap();

    assert_eq!(state.lock().unwrap().begin_mute_call_count, 1);
}

#[test]
fn mute_and_unmute_preserve_user_muted_audio() {
    let (state, audio_mute_manager) = manager_with(FakeAudioControllerState {
        is_muted: true,
        ..Default::default()
    });

    audio_mute_manager.mute().unwrap();
    audio_mute_manager.unmute().unwrap();

    let state_after_operations = state.lock().unwrap();
    assert_eq!(state_after_operations.begin_mute_call_count, 0);
    assert_eq!(state_after_operations.end_mute_call_count, 0);
    assert!(state_after_operations.is_muted);
}

#[test]
fn mute_falls_back_to_not_muted_when_is_muted_query_fails() {
    let (state, audio_mute_manager) = manager_with(FakeAudioControllerState {
        is_muted_error: Some("query failure".to_string()),
        ..Default::default()
    });

    audio_mute_manager.mute().unwrap();

    assert_eq!(state.lock().unwrap().begin_mute_call_count, 1);
}

#[test]
fn unmute_failure_keeps_muted_state_for_retry() {
    let (state, audio_mute_manager) = manager_with(FakeAudioControllerState {
        end_mute_error: Some("restore failure".to_string()),
        ..Default::default()
    });

    audio_mute_manager.mute().unwrap();
    assert!(audio_mute_manager.unmute().is_err());
    assert_eq!(audio_mute_manager.current_state(), MuteState::MutedByUs);

    state.lock().unwrap().end_mute_error = None;

    audio_mute_manager.unmute().unwrap();
    assert_eq!(audio_mute_manager.current_state(), MuteState::NotMuting);

    let state_after_retry = state.lock().unwrap();
    assert_eq!(state_after_retry.end_mute_call_count, 2);
    assert!(!state_after_retry.is_muted);
}

#[test]
fn mute_failure_with_recovery_session_keeps_recovery_state_until_unmute() {
    let (state, audio_mute_manager) = manager_with(FakeAudioControllerState {
        begin_mute_error_with_recovery_session: Some(
            "partial mute and rollback failed".to_string(),
        ),
        ..Default::default()
    });

    assert!(audio_mute_manager.mute().is_err());
    assert_eq!(
        audio_mute_manager.current_state(),
        MuteState::RecoveryPending
    );

    {
        let state_after_failed_mute = state.lock().unwrap();
        assert_eq!(state_after_failed_mute.begin_mute_call_count, 1);
        assert_eq!(state_after_failed_mute.end_mute_call_count, 0);
        assert!(state_after_failed_mute.is_muted);
    }

    audio_mute_manager.unmute().unwrap();
    assert_eq!(audio_mute_manager.current_state(), MuteState::NotMuting);

    let state_after_recovery = state.lock().unwrap();
    assert_eq!(state_after_recovery.end_mute_call_count, 1);
    assert!(!state_after_recovery.is_muted);
}

#[test]
fn mute_returns_error_while_recovery_is_pending() {
    let (_state, audio_mute_manager) = manager_with(FakeAudioControllerState {
        begin_mute_error_with_recovery_session: Some(
            "partial mute and rollback failed".to_string(),
        ),
        ..Default::default()
    });

    assert!(audio_mute_manager.mute().is_err());
    let second_mute_result = audio_mute_manager.mute();
    assert!(matches!(
        second_mute_result,
        Err(AudioControlError::SetPropertyFailed(error_message))
        if error_message.contains("recovery is pending")
    ));
}

#[test]
fn drop_unmutes_when_manager_muted_audio() {
    let state = Arc::new(Mutex::new(FakeAudioControllerState::default()));
    {
        let audio_mute_manager =
            AudioMuteManager::from_controller(Box::new(FakeAudioController::new(state.clone())));
        audio_mute_manager.mute().unwrap();
    }

    let state_after_drop = state.lock().unwrap();
    assert_eq!(state_after_drop.begin_mute_call_count, 1);
    assert_eq!(state_after_drop.end_mute_call_count, 1);
    assert!(!state_after_drop.is_muted);
}

fn build_output_volume_scalar_snapshot(property_element: u32) -> OutputVolumeScalarSnapshot {
    OutputVolumeScalarSnapshot {
        property_element,
        initial_volume_scalar: 0.5,
    }
}

#[test]
fn select_mute_strategy_prefers_device_mute_property_when_supported_and_settable() {
    assert_eq!(
        select_mute_strategy(true, true),
        MuteStrategy::DeviceMuteProperty
    );
}

#[test]
fn select_mute_strategy_falls_back_to_volume_zero_when_mute_property_is_not_settable() {
    assert_eq!(
        select_mute_strategy(true, false),
        MuteStrategy::VolumeZeroFallback
    );
    assert_eq!(
        select_mute_strategy(false, false),
        MuteStrategy::VolumeZeroFallback
    );
}

#[test]
fn volume_zero_fallback_outcome_is_success_when_all_settable_targets_are_muted() {
    let outcome = evaluate_volume_zero_fallback_mute_attempt(
        vec![
            build_output_volume_scalar_snapshot(1),
            build_output_volume_scalar_snapshot(2),
        ],
        2,
        None,
        None,
    );
    assert!(matches!(
        outcome,
        VolumeZeroFallbackMuteAttemptResult::MutedAllSettable { .. }
    ));
}

#[test]
fn volume_zero_fallback_outcome_fails_when_no_settable_targets_are_available() {
    let outcome = evaluate_volume_zero_fallback_mute_attempt(Vec::new(), 0, None, None);
    assert!(matches!(
        outcome,
        VolumeZeroFallbackMuteAttemptResult::FailedToMuteAllSettable { .. }
    ));
}

#[test]
fn volume_zero_fallback_outcome_requires_rollback_when_capture_probe_reports_an_error() {
    let capture_error = AudioControlError::GetPropertyFailed("OSStatus: -1".to_string());
    let outcome = evaluate_volume_zero_fallback_mute_attempt(
        vec![
            build_output_volume_scalar_snapshot(1),
            build_output_volume_scalar_snapshot(2),
        ],
        2,
        Some(capture_error),
        None,
    );
    assert!(matches!(
        outcome,
        VolumeZeroFallbackMuteAttemptResult::FailedToMuteAllSettable { .. }
    ));
}

#[test]
fn volume_zero_fallback_outcome_requires_rollback_when_apply_reports_an_error() {
    let apply_error = AudioControlError::SetPropertyFailed("OSStatus: -1".to_string());
    let outcome = evaluate_volume_zero_fallback_mute_attempt(
        vec![
            build_output_volume_scalar_snapshot(1),
            build_output_volume_scalar_snapshot(2),
        ],
        2,
        None,
        Some(apply_error),
    );
    assert!(matches!(
        outcome,
        VolumeZeroFallbackMuteAttemptResult::FailedToMuteAllSettable { .. }
    ));
}

// ---------------------------------------------------------------------------
// Wispr Bro

#[test]
fn a_take_after_the_user_unmuted_is_muted_again() {
    // Handy never calls unmute after a mute that found the audio muted.
    let (state, m) = manager_with(FakeAudioControllerState {
        is_muted: true,
        ..Default::default()
    });
    m.mute().unwrap();
    assert_eq!(m.current_state(), MuteState::AudioWasAlreadyMutedByUser);
    // The user unmutes between takes; the next take mutes.
    state.lock().unwrap().is_muted = false;
    m.mute().unwrap();
    assert_eq!(m.current_state(), MuteState::MutedByUs);
    assert_eq!(state.lock().unwrap().begin_mute_call_count, 1);
    m.unmute().unwrap();
    assert!(!state.lock().unwrap().is_muted);
}

#[test]
fn an_unmute_without_our_mute_touches_nothing() {
    let (state, m) = manager_with(FakeAudioControllerState {
        is_muted: true,
        ..Default::default()
    });
    m.unmute().unwrap();
    let s = state.lock().unwrap();
    assert!(s.is_muted);
    assert_eq!(s.end_mute_call_count, 0);
}

/// Output devices with a mute flag or only a volume, and a default that can
/// change mid-take, like the HAL.
#[derive(Debug, Clone, PartialEq)]
struct FakeDevice {
    has_mute: bool,
    muted: bool,
    volume: f32,
}

#[derive(Default)]
struct FakeHal {
    devices: BTreeMap<u32, FakeDevice>,
    default_output: u32,
}

struct DevicesController(Arc<Mutex<FakeHal>>);

impl SystemAudioControl for DevicesController {
    fn is_muted(&self) -> Result<bool, AudioControlError> {
        let hal = self.0.lock().unwrap();
        Ok(hal.devices[&hal.default_output].muted)
    }

    fn begin_mute_session(&self) -> Result<ActiveMuteSession, AudioControlError> {
        let mut hal = self.0.lock().unwrap();
        let id = hal.default_output;
        let d = hal.devices.get_mut(&id).unwrap();
        if d.has_mute {
            let initial = d.muted;
            d.muted = true;
            Ok(ActiveMuteSession::MacOsDeviceMute {
                output_device_id: id,
                initial_device_muted: initial,
            })
        } else {
            let saved = d.volume;
            d.volume = 0.0;
            Ok(ActiveMuteSession::MacOsVolumeZeroFallback {
                output_device_id: id,
                captured_volume_scalars: vec![OutputVolumeScalarSnapshot {
                    property_element: 0,
                    initial_volume_scalar: saved,
                }],
            })
        }
    }

    fn end_mute_session(&self, s: &ActiveMuteSession) -> Result<(), AudioControlError> {
        let mut hal = self.0.lock().unwrap();
        match s {
            ActiveMuteSession::MacOsDeviceMute {
                output_device_id,
                initial_device_muted,
            } => hal.devices.get_mut(output_device_id).unwrap().muted = *initial_device_muted,
            ActiveMuteSession::MacOsVolumeZeroFallback {
                output_device_id,
                captured_volume_scalars,
            } => {
                hal.devices.get_mut(output_device_id).unwrap().volume =
                    captured_volume_scalars[0].initial_volume_scalar
            }
        }
        Ok(())
    }
}

fn hal(devices: &[(u32, FakeDevice)], default_output: u32) -> Arc<Mutex<FakeHal>> {
    Arc::new(Mutex::new(FakeHal {
        devices: devices.iter().cloned().collect(),
        default_output,
    }))
}

#[test]
fn the_device_that_was_muted_is_restored_even_after_the_default_changed() {
    let speakers = FakeDevice {
        has_mute: true,
        muted: false,
        volume: 0.37,
    };
    let airpods = FakeDevice {
        has_mute: true,
        muted: false,
        volume: 0.8,
    };
    let h = hal(&[(1, speakers.clone()), (2, airpods.clone())], 1);
    let m = AudioMuteManager::from_controller(Box::new(DevicesController(h.clone())));
    m.mute().unwrap();
    assert!(h.lock().unwrap().devices[&1].muted);
    // AirPods connect mid-take and become the default output.
    h.lock().unwrap().default_output = 2;
    m.unmute().unwrap();
    let after = h.lock().unwrap();
    assert_eq!(after.devices[&1], speakers, "speakers back exactly");
    assert_eq!(after.devices[&2], airpods, "AirPods never touched");
}

#[test]
fn a_device_without_a_mute_flag_gets_its_exact_volume_back() {
    let hdmi = FakeDevice {
        has_mute: false,
        muted: false,
        volume: 0.37,
    };
    let h = hal(&[(7, hdmi.clone())], 7);
    let m = AudioMuteManager::from_controller(Box::new(DevicesController(h.clone())));
    m.mute().unwrap();
    assert_eq!(h.lock().unwrap().devices[&7].volume, 0.0);
    m.unmute().unwrap();
    assert_eq!(h.lock().unwrap().devices[&7], hdmi);
    assert_eq!(
        h.lock().unwrap().devices[&7].volume.to_bits(),
        0.37f32.to_bits()
    );
}

#[test]
fn a_device_muted_before_the_take_stays_muted() {
    let speakers = FakeDevice {
        has_mute: true,
        muted: true,
        volume: 0.5,
    };
    let h = hal(&[(1, speakers.clone())], 1);
    let m = AudioMuteManager::from_controller(Box::new(DevicesController(h.clone())));
    m.mute().unwrap();
    m.unmute().unwrap();
    assert_eq!(h.lock().unwrap().devices[&1], speakers);
}

#[test]
fn handys_macos_mute_calls_never_spawn_osascript() {
    // R5 S-08 exit criterion: no `osascript` anywhere in a take or at mute.
    let src = include_str!("../../managers/audio.rs");
    assert!(
        !src.contains("osascript"),
        "managers/audio.rs must not run AppleScript"
    );
    assert!(src.contains("crate::fork::audio::mute::set_mute(mute)"));
    assert!(src.contains("crate::fork::audio::mute::get_mute()"));
}
