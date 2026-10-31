// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) the Tambourine authors (https://github.com/kstonekuan/tambourine-voice)
// Modified by Wispr Bro contributors, 2026-10-31.
//
// Ported from Tambourine `app/src-tauri/src/audio_mute/mod.rs`,
// `audio_mute/macos.rs` and `src/tests/audio_mute_tests.rs`
// (https://github.com/kstonekuan/tambourine-voice, commit
// c54b527d5920b821e1f0ff837457d69405696b6b), licensed AGPL-3.0 (R4 addendum
// W-29). Kept: the controller trait and its sessions, the mute manager's
// state machine (muted by us, already muted by the user, recovery pending
// after a failed rollback), the CoreAudio controller (the device mute
// property, else every settable volume control at zero with its scalar
// saved, rolled back on a partial failure), and all their tests. Changed:
// the Windows and stub controllers are not ported (Handy keeps its own code
// there); log lines are plain debug/info lines without device names; a
// session left "already muted by the user" is re-checked at the next mute
// (Handy never calls unmute after a mute that found the audio muted, so the
// state would otherwise stick and a later take would not mute); a fake
// controller with several devices tests that a session restores the device
// it muted even when the default output changed meanwhile; Handy's
// `set_mute`/`get_mute` hooks ([`set_mute`], [`get_mute`]) and the exit
// restore ([`restore_on_exit`]) are ours.
//! "Mute all audio while dictating" without AppleScript (R5 S-08, W-29).
//!
//! Handy mutes the system output while recording when its
//! `mute_while_recording` setting is on (off by default). Its macOS backend
//! ran `osascript` twice per take; `managers/audio.rs` now calls [`set_mute`]
//! and [`get_mute`] instead (two one-line hooks). A mute is a session: the
//! output device and what was changed on it (its mute flag, or its volume
//! scalars when it has no settable mute) are saved, and the end of the take
//! puts exactly that back on exactly that device, even if the default output
//! changed during the take (AirPods connecting, say). If the user had muted
//! the audio already, nothing is touched and it stays muted.

use std::fmt;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq)]
pub struct OutputVolumeScalarSnapshot {
    pub property_element: u32,
    pub initial_volume_scalar: f32,
}

/// What a mute changed, so that exactly that can be put back.
#[derive(Debug, Clone, PartialEq)]
pub enum ActiveMuteSession {
    /// The device's mute property was set; it was `initial_device_muted`.
    MacOsDeviceMute {
        output_device_id: u32,
        initial_device_muted: bool,
    },
    /// The device has no settable mute: its volume controls were set to 0.
    MacOsVolumeZeroFallback {
        output_device_id: u32,
        captured_volume_scalars: Vec<OutputVolumeScalarSnapshot>,
    },
}

impl ActiveMuteSession {
    fn telemetry_summary(&self) -> String {
        match self {
            Self::MacOsDeviceMute {
                output_device_id, ..
            } => format!("device_mute(output_device_id={output_device_id})"),
            Self::MacOsVolumeZeroFallback {
                output_device_id,
                captured_volume_scalars,
            } => format!(
                "volume_zero_fallback(output_device_id={output_device_id},captured_scalar_count={})",
                captured_volume_scalars.len()
            ),
        }
    }
}

/// Error type for audio control operations.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Some variants exist only on macOS.
pub enum AudioControlError {
    InitializationFailed(String),
    GetPropertyFailed(String),
    SetPropertyFailed(String),
    /// Failed to start a mute session. May carry a session that restores
    /// what was changed before the failure.
    MuteSessionStartFailed {
        message: String,
        recovery_session: Option<ActiveMuteSession>,
    },
    NotSupported,
}

impl AudioControlError {
    fn recovery_session(&self) -> Option<&ActiveMuteSession> {
        match self {
            Self::MuteSessionStartFailed {
                recovery_session: Some(active_mute_session),
                ..
            } => Some(active_mute_session),
            _ => None,
        }
    }
}

impl fmt::Display for AudioControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitializationFailed(message) => write!(f, "Audio init failed: {message}"),
            Self::GetPropertyFailed(message) => {
                write!(f, "Failed to get audio property: {message}")
            }
            Self::SetPropertyFailed(message) => {
                write!(f, "Failed to set audio property: {message}")
            }
            Self::MuteSessionStartFailed { message, .. } => {
                write!(f, "Failed to start mute session: {message}")
            }
            Self::NotSupported => write!(f, "Audio control not supported on this platform"),
        }
    }
}

impl std::error::Error for AudioControlError {}

/// Controls the system output's mute state.
pub trait SystemAudioControl: Send + Sync {
    /// Whether the default output is muted.
    fn is_muted(&self) -> Result<bool, AudioControlError>;

    /// Mutes the default output and returns what is needed to restore it.
    fn begin_mute_session(&self) -> Result<ActiveMuteSession, AudioControlError>;

    /// Restores what `active_mute_session` changed.
    fn end_mute_session(
        &self,
        active_mute_session: &ActiveMuteSession,
    ) -> Result<(), AudioControlError>;
}

/// The platform controller.
pub fn create_controller() -> Result<Box<dyn SystemAudioControl>, AudioControlError> {
    #[cfg(target_os = "macos")]
    {
        macos::MacOSAudioController::new()
            .map(|controller| Box::new(controller) as Box<dyn SystemAudioControl>)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(AudioControlError::NotSupported)
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MuteState {
    #[default]
    NotMuting,
    MutedByUs,
    RecoveryPending,
    AudioWasAlreadyMutedByUser,
}

#[derive(Debug, Clone, PartialEq, Default)]
enum AudioMuteManagerState {
    #[default]
    NotMuting,
    MutedByUs {
        active_mute_session: ActiveMuteSession,
    },
    RecoveryPending {
        active_mute_session: ActiveMuteSession,
        mute_error_message: String,
    },
    AudioWasAlreadyMutedByUser,
}

impl AudioMuteManagerState {
    #[cfg(test)]
    fn as_public_mute_state(&self) -> MuteState {
        match self {
            Self::NotMuting => MuteState::NotMuting,
            Self::MutedByUs { .. } => MuteState::MutedByUs,
            Self::RecoveryPending { .. } => MuteState::RecoveryPending,
            Self::AudioWasAlreadyMutedByUser => MuteState::AudioWasAlreadyMutedByUser,
        }
    }
}

/// Mutes and unmutes the system audio around a take.
pub struct AudioMuteManager {
    controller: Box<dyn SystemAudioControl>,
    state: Mutex<AudioMuteManagerState>,
}

impl AudioMuteManager {
    pub fn new() -> Option<Self> {
        match create_controller() {
            Ok(controller) => Some(Self::from_controller(controller)),
            Err(error) => {
                log::warn!("fork audio: mute while dictating is not available: {error}");
                None
            }
        }
    }

    pub fn from_controller(controller: Box<dyn SystemAudioControl>) -> Self {
        Self {
            controller,
            state: Mutex::new(AudioMuteManagerState::NotMuting),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, AudioMuteManagerState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[cfg(test)]
    pub(crate) fn current_state(&self) -> MuteState {
        self.lock().as_public_mute_state()
    }

    /// Whether the system output is muted right now.
    pub fn is_muted(&self) -> Result<bool, AudioControlError> {
        self.controller.is_muted()
    }

    pub fn mute(&self) -> Result<(), AudioControlError> {
        let mut state_guard = self.lock();

        match &*state_guard {
            // Wispr Bro: re-check, the user may have unmuted since.
            AudioMuteManagerState::NotMuting
            | AudioMuteManagerState::AudioWasAlreadyMutedByUser => {}
            AudioMuteManagerState::MutedByUs { .. } => return Ok(()),
            AudioMuteManagerState::RecoveryPending {
                active_mute_session,
                mute_error_message,
            } => {
                let blocked_mute_error_message = format!(
                    "Cannot start a new mute operation while recovery is pending from a previous mute failure: {mute_error_message}"
                );
                log::warn!(
                    "fork audio: mute blocked, recovery pending (session {})",
                    active_mute_session.telemetry_summary()
                );
                return Err(AudioControlError::SetPropertyFailed(
                    blocked_mute_error_message,
                ));
            }
        }

        let audio_is_already_muted = self.controller.is_muted().unwrap_or(false);
        if audio_is_already_muted {
            *state_guard = AudioMuteManagerState::AudioWasAlreadyMutedByUser;
            log::info!("fork audio: system audio already muted; left as it is");
            return Ok(());
        }

        match self.controller.begin_mute_session() {
            Ok(active_mute_session) => {
                log::info!(
                    "fork audio: system audio muted for the take ({})",
                    active_mute_session.telemetry_summary()
                );
                *state_guard = AudioMuteManagerState::MutedByUs {
                    active_mute_session,
                };
                Ok(())
            }
            Err(begin_mute_error) => {
                if let Some(recovery_session) = begin_mute_error.recovery_session() {
                    log::warn!(
                        "fork audio: mute failed, recovery pending ({}): {begin_mute_error}",
                        recovery_session.telemetry_summary()
                    );
                    *state_guard = AudioMuteManagerState::RecoveryPending {
                        active_mute_session: recovery_session.clone(),
                        mute_error_message: begin_mute_error.to_string(),
                    };
                }
                Err(begin_mute_error)
            }
        }
    }

    pub fn unmute(&self) -> Result<(), AudioControlError> {
        let mut state_guard = self.lock();

        match &*state_guard {
            AudioMuteManagerState::MutedByUs {
                active_mute_session,
            }
            | AudioMuteManagerState::RecoveryPending {
                active_mute_session,
                ..
            } => {
                let summary = active_mute_session.telemetry_summary();
                if let Err(end_mute_error) = self.controller.end_mute_session(active_mute_session) {
                    log::warn!(
                        "fork audio: restoring the audio failed ({summary}): {end_mute_error}"
                    );
                    return Err(end_mute_error);
                }
                *state_guard = AudioMuteManagerState::NotMuting;
                log::info!("fork audio: system audio restored after the take ({summary})");
            }
            AudioMuteManagerState::AudioWasAlreadyMutedByUser => {
                *state_guard = AudioMuteManagerState::NotMuting;
                log::info!("fork audio: system audio was already muted; leaving it muted");
            }
            AudioMuteManagerState::NotMuting => {}
        }

        Ok(())
    }

    /// `true` while a session of ours is open (muted, or recovery pending).
    pub fn holds_a_session(&self) -> bool {
        matches!(
            *self.lock(),
            AudioMuteManagerState::MutedByUs { .. } | AudioMuteManagerState::RecoveryPending { .. }
        )
    }
}

impl Drop for AudioMuteManager {
    fn drop(&mut self) {
        // Try to restore on drop (exit).
        if self.holds_a_session() {
            if let Err(drop_cleanup_error) = self.unmute() {
                log::warn!("fork audio: restore at drop failed: {drop_cleanup_error}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Handy's hooks (Wispr Bro)

fn manager() -> Option<&'static AudioMuteManager> {
    static MANAGER: OnceLock<Option<AudioMuteManager>> = OnceLock::new();
    MANAGER.get_or_init(AudioMuteManager::new).as_ref()
}

/// Hook in Handy's `managers/audio.rs` `set_mute` (macOS): `true` starts a
/// mute session for the take, `false` ends it. Handy calls `false` only
/// after a `true` that found the audio unmuted (or unknown); a `false` with
/// no session of ours touches nothing, so a user's own mute is never undone.
pub fn set_mute(mute: bool) {
    let Some(m) = manager() else { return };
    let result = if mute { m.mute() } else { m.unmute() };
    if let Err(e) = result {
        log::warn!("fork audio: mute while dictating: {e}");
    }
}

/// Hook in Handy's `managers/audio.rs` `get_mute` (macOS): the default
/// output's mute flag, or `None` when it cannot be read.
pub fn get_mute() -> Option<bool> {
    manager()?.is_muted().ok()
}

/// From `RunEvent::Exit`: never leave the audio muted by us.
pub fn restore_on_exit() {
    if let Some(m) = manager() {
        if m.holds_a_session() {
            log::info!("fork audio: restoring the system audio at exit");
            if let Err(e) = m.unmute() {
                log::warn!("fork audio: restore at exit failed: {e}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// macOS controller (CoreAudio)

#[cfg(target_os = "macos")]
mod macos {
    use super::{
        ActiveMuteSession, AudioControlError, OutputVolumeScalarSnapshot, SystemAudioControl,
    };
    use objc2_core_audio::{
        kAudioDevicePropertyMute, kAudioDevicePropertyPreferredChannelsForStereo,
        kAudioDevicePropertyScopeOutput, kAudioDevicePropertyVolumeScalar,
        kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyElementMain,
        kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, AudioObjectGetPropertyData,
        AudioObjectHasProperty, AudioObjectIsPropertySettable, AudioObjectPropertyAddress,
        AudioObjectSetPropertyData,
    };
    use std::ffi::c_void;
    use std::ptr::NonNull;

    use super::pure::{
        evaluate_volume_zero_fallback_mute_attempt, select_mute_strategy, MuteStrategy,
        VolumeZeroFallbackMuteAttemptResult,
    };

    /// macOS audio controller using CoreAudio.
    pub struct MacOSAudioController;

    impl MacOSAudioController {
        pub fn new() -> Result<Self, AudioControlError> {
            let _ = Self::get_default_output_device()?;
            Ok(Self)
        }

        fn get_default_output_device() -> Result<u32, AudioControlError> {
            let address = AudioObjectPropertyAddress {
                mSelector: kAudioHardwarePropertyDefaultOutputDevice,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain,
            };

            let mut device_id: u32 = 0;
            let mut size = size_of::<u32>() as u32;

            // SAFETY: `device_id` has room for the `size` bytes written.
            let status = unsafe {
                AudioObjectGetPropertyData(
                    kAudioObjectSystemObject as u32,
                    NonNull::from(&address),
                    0,
                    std::ptr::null(),
                    NonNull::from(&mut size),
                    NonNull::from(&mut device_id).cast::<c_void>(),
                )
            };

            if status != 0 {
                return Err(AudioControlError::InitializationFailed(format!(
                    "Failed to get default output device (OSStatus: {status})"
                )));
            }

            if device_id == 0 {
                return Err(AudioControlError::InitializationFailed(
                    "No default output device found".to_string(),
                ));
            }

            Ok(device_id)
        }

        fn build_output_property_address(
            selector: u32,
            element: u32,
        ) -> AudioObjectPropertyAddress {
            AudioObjectPropertyAddress {
                mSelector: selector,
                mScope: kAudioDevicePropertyScopeOutput,
                mElement: element,
            }
        }

        fn has_property(device_id: u32, selector: u32, element: u32) -> bool {
            let address = Self::build_output_property_address(selector, element);
            // SAFETY: the address outlives the call.
            unsafe { AudioObjectHasProperty(device_id, NonNull::from(&address)) }
        }

        fn is_property_settable(
            device_id: u32,
            selector: u32,
            element: u32,
        ) -> Result<bool, AudioControlError> {
            let address = Self::build_output_property_address(selector, element);
            let mut is_settable: u8 = 0;
            // SAFETY: the address and the out value outlive the call.
            let status = unsafe {
                AudioObjectIsPropertySettable(
                    device_id,
                    NonNull::from(&address),
                    NonNull::from(&mut is_settable),
                )
            };

            if status != 0 {
                return Err(AudioControlError::GetPropertyFailed(format!(
                    "Failed to query property settable status (OSStatus: {status})"
                )));
            }

            Ok(is_settable != 0)
        }

        fn get_u32_property(
            device_id: u32,
            selector: u32,
            element: u32,
        ) -> Result<u32, AudioControlError> {
            let address = Self::build_output_property_address(selector, element);
            let mut value: u32 = 0;
            let mut size = size_of::<u32>() as u32;

            // SAFETY: `value` has room for the `size` bytes written.
            let status = unsafe {
                AudioObjectGetPropertyData(
                    device_id,
                    NonNull::from(&address),
                    0,
                    std::ptr::null(),
                    NonNull::from(&mut size),
                    NonNull::from(&mut value).cast::<c_void>(),
                )
            };

            if status != 0 {
                return Err(AudioControlError::GetPropertyFailed(format!(
                    "OSStatus: {status}"
                )));
            }

            Ok(value)
        }

        fn set_u32_property(
            device_id: u32,
            selector: u32,
            element: u32,
            value: u32,
        ) -> Result<(), AudioControlError> {
            let address = Self::build_output_property_address(selector, element);
            let size = size_of::<u32>() as u32;

            // SAFETY: `value` holds `size` bytes; CoreAudio only reads them.
            let status = unsafe {
                AudioObjectSetPropertyData(
                    device_id,
                    NonNull::from(&address),
                    0,
                    std::ptr::null(),
                    size,
                    NonNull::from(&value).cast::<c_void>(),
                )
            };

            if status != 0 {
                return Err(AudioControlError::SetPropertyFailed(format!(
                    "OSStatus: {status}"
                )));
            }

            Ok(())
        }

        fn get_f32_property(
            device_id: u32,
            selector: u32,
            element: u32,
        ) -> Result<f32, AudioControlError> {
            let address = Self::build_output_property_address(selector, element);
            let mut value: f32 = 0.0;
            let mut size = size_of::<f32>() as u32;

            // SAFETY: `value` has room for the `size` bytes written.
            let status = unsafe {
                AudioObjectGetPropertyData(
                    device_id,
                    NonNull::from(&address),
                    0,
                    std::ptr::null(),
                    NonNull::from(&mut size),
                    NonNull::from(&mut value).cast::<c_void>(),
                )
            };

            if status != 0 {
                return Err(AudioControlError::GetPropertyFailed(format!(
                    "OSStatus: {status}"
                )));
            }

            Ok(value)
        }

        fn set_f32_property(
            device_id: u32,
            selector: u32,
            element: u32,
            value: f32,
        ) -> Result<(), AudioControlError> {
            let address = Self::build_output_property_address(selector, element);
            let size = size_of::<f32>() as u32;

            // SAFETY: `value` holds `size` bytes; CoreAudio only reads them.
            let status = unsafe {
                AudioObjectSetPropertyData(
                    device_id,
                    NonNull::from(&address),
                    0,
                    std::ptr::null(),
                    size,
                    NonNull::from(&value).cast::<c_void>(),
                )
            };

            if status != 0 {
                return Err(AudioControlError::SetPropertyFailed(format!(
                    "OSStatus: {status}"
                )));
            }

            Ok(())
        }

        fn get_preferred_stereo_channels(device_id: u32) -> Result<[u32; 2], AudioControlError> {
            let address = Self::build_output_property_address(
                kAudioDevicePropertyPreferredChannelsForStereo,
                kAudioObjectPropertyElementMain,
            );
            let mut channels = [0u32; 2];
            let mut size = size_of_val(&channels) as u32;

            // SAFETY: `channels` has room for the `size` bytes written.
            let status = unsafe {
                AudioObjectGetPropertyData(
                    device_id,
                    NonNull::from(&address),
                    0,
                    std::ptr::null(),
                    NonNull::from(&mut size),
                    NonNull::from(&mut channels).cast::<c_void>(),
                )
            };

            if status != 0 {
                return Err(AudioControlError::GetPropertyFailed(format!(
                    "Failed to get preferred stereo channels (OSStatus: {status})"
                )));
            }

            Ok(channels)
        }

        fn select_mute_strategy_for_device(device_id: u32) -> MuteStrategy {
            let mute_property_exists = Self::has_property(
                device_id,
                kAudioDevicePropertyMute,
                kAudioObjectPropertyElementMain,
            );
            let mute_property_is_settable = mute_property_exists
                && Self::is_property_settable(
                    device_id,
                    kAudioDevicePropertyMute,
                    kAudioObjectPropertyElementMain,
                )
                .unwrap_or(false);

            select_mute_strategy(mute_property_exists, mute_property_is_settable)
        }

        fn build_volume_zero_fallback_targets(device_id: u32) -> Vec<(u32, String)> {
            let mut volume_zero_fallback_targets =
                vec![(kAudioObjectPropertyElementMain, "main output".to_string())];

            match Self::get_preferred_stereo_channels(device_id) {
                Ok(preferred_stereo_channels) => {
                    for (stereo_channel_index, stereo_channel_element) in
                        preferred_stereo_channels.into_iter().enumerate()
                    {
                        if volume_zero_fallback_targets
                            .iter()
                            .any(|(existing_element, _)| {
                                *existing_element == stereo_channel_element
                            })
                        {
                            continue;
                        }

                        let stereo_channel_label = if stereo_channel_index == 0 {
                            "left stereo channel"
                        } else {
                            "right stereo channel"
                        };
                        volume_zero_fallback_targets
                            .push((stereo_channel_element, stereo_channel_label.to_string()));
                    }
                }
                Err(error) => {
                    log::debug!(
                        "fork audio: no preferred stereo channels on device {device_id}: {error}"
                    );
                }
            }

            volume_zero_fallback_targets
        }

        fn capture_settable_volume_targets_with_snapshot_scalars(
            device_id: u32,
            volume_zero_fallback_targets: Vec<(u32, String)>,
        ) -> (Vec<(u32, String, f32)>, Option<AudioControlError>) {
            let mut latest_volume_zero_error: Option<AudioControlError> = None;
            let mut volume_targets_with_snapshot_scalars: Vec<(u32, String, f32)> = Vec::new();

            for (volume_element, volume_element_label) in volume_zero_fallback_targets {
                match Self::is_property_settable(
                    device_id,
                    kAudioDevicePropertyVolumeScalar,
                    volume_element,
                ) {
                    Ok(true) => {}
                    Ok(false) => continue,
                    Err(error) => {
                        log::debug!(
                            "fork audio: is the {volume_element_label} volume settable on device {device_id}: {error}"
                        );
                        latest_volume_zero_error = Some(error);
                        continue;
                    }
                }

                let initial_volume_scalar = match Self::get_f32_property(
                    device_id,
                    kAudioDevicePropertyVolumeScalar,
                    volume_element,
                ) {
                    Ok(initial_volume_scalar) => initial_volume_scalar,
                    Err(error) => {
                        log::debug!(
                            "fork audio: cannot read the {volume_element_label} volume on device {device_id}: {error}"
                        );
                        latest_volume_zero_error = Some(error);
                        continue;
                    }
                };

                volume_targets_with_snapshot_scalars.push((
                    volume_element,
                    volume_element_label,
                    initial_volume_scalar,
                ));
            }

            (
                volume_targets_with_snapshot_scalars,
                latest_volume_zero_error,
            )
        }

        fn apply_volume_zero_to_captured_targets(
            device_id: u32,
            volume_targets_with_snapshot_scalars: Vec<(u32, String, f32)>,
        ) -> (Vec<OutputVolumeScalarSnapshot>, Option<AudioControlError>) {
            let mut latest_volume_zero_error: Option<AudioControlError> = None;
            let mut captured_volume_scalars: Vec<OutputVolumeScalarSnapshot> = Vec::new();

            for (volume_element, volume_element_label, initial_volume_scalar) in
                volume_targets_with_snapshot_scalars
            {
                match Self::set_f32_property(
                    device_id,
                    kAudioDevicePropertyVolumeScalar,
                    volume_element,
                    0.0,
                ) {
                    Ok(()) => {
                        captured_volume_scalars.push(OutputVolumeScalarSnapshot {
                            property_element: volume_element,
                            initial_volume_scalar,
                        });
                    }
                    Err(error) => {
                        log::debug!(
                            "fork audio: cannot set the {volume_element_label} volume to zero on device {device_id}: {error}"
                        );
                        latest_volume_zero_error = Some(error);
                    }
                }
            }

            (captured_volume_scalars, latest_volume_zero_error)
        }

        fn apply_volume_zero_fallback_and_capture_snapshot(
            device_id: u32,
        ) -> Result<Vec<OutputVolumeScalarSnapshot>, AudioControlError> {
            let volume_zero_fallback_targets = Self::build_volume_zero_fallback_targets(device_id);
            let (volume_targets_with_snapshot_scalars, latest_capture_error) =
                Self::capture_settable_volume_targets_with_snapshot_scalars(
                    device_id,
                    volume_zero_fallback_targets,
                );
            let total_settable_target_count = volume_targets_with_snapshot_scalars.len();
            let (captured_volume_scalars, latest_apply_error) =
                Self::apply_volume_zero_to_captured_targets(
                    device_id,
                    volume_targets_with_snapshot_scalars,
                );
            match evaluate_volume_zero_fallback_mute_attempt(
                captured_volume_scalars,
                total_settable_target_count,
                latest_capture_error,
                latest_apply_error,
            ) {
                VolumeZeroFallbackMuteAttemptResult::MutedAllSettable {
                    captured_volume_scalars,
                } => Ok(captured_volume_scalars),
                VolumeZeroFallbackMuteAttemptResult::FailedToMuteAllSettable {
                    captured_volume_scalars,
                    total_settable_target_count,
                    mute_error,
                } => {
                    if captured_volume_scalars.is_empty() {
                        return Err(mute_error);
                    }

                    match Self::restore_volume_zero_fallback_session(
                        device_id,
                        &captured_volume_scalars,
                    ) {
                        Ok(()) => Err(mute_error),
                        Err(rollback_error) => {
                            log::warn!(
                                "fork audio: volume-zero rollback failed: device_id={} muted_control_count={} total_settable_control_count={} rollback_error={}",
                                device_id,
                                captured_volume_scalars.len(),
                                total_settable_target_count,
                                rollback_error,
                            );
                            Err(AudioControlError::MuteSessionStartFailed {
                                message: format!(
                                    "{mute_error}. Failed to roll back partially-applied volume-zero fallback on device {device_id}: {rollback_error}"
                                ),
                                recovery_session: Some(ActiveMuteSession::MacOsVolumeZeroFallback {
                                    output_device_id: device_id,
                                    captured_volume_scalars,
                                }),
                            })
                        }
                    }
                }
            }
        }

        fn restore_device_mute_session(
            output_device_id: u32,
            initial_device_muted: bool,
        ) -> Result<(), AudioControlError> {
            Self::set_u32_property(
                output_device_id,
                kAudioDevicePropertyMute,
                kAudioObjectPropertyElementMain,
                u32::from(initial_device_muted),
            )
        }

        fn restore_volume_zero_fallback_session(
            output_device_id: u32,
            captured_volume_scalars: &[OutputVolumeScalarSnapshot],
        ) -> Result<(), AudioControlError> {
            let mut latest_restore_error: Option<AudioControlError> = None;
            let mut successfully_restored_volume_count = 0usize;

            for captured_volume_scalar in captured_volume_scalars {
                match Self::set_f32_property(
                    output_device_id,
                    kAudioDevicePropertyVolumeScalar,
                    captured_volume_scalar.property_element,
                    captured_volume_scalar.initial_volume_scalar,
                ) {
                    Ok(()) => successfully_restored_volume_count += 1,
                    Err(error) => {
                        log::warn!(
                            "fork audio: cannot restore the volume of element {}: {error}",
                            captured_volume_scalar.property_element
                        );
                        latest_restore_error = Some(error);
                    }
                }
            }

            if latest_restore_error.is_none()
                && !captured_volume_scalars.is_empty()
                && successfully_restored_volume_count == captured_volume_scalars.len()
            {
                return Ok(());
            }

            if captured_volume_scalars.is_empty() {
                return Err(AudioControlError::SetPropertyFailed(
                    "Failed to restore macOS fallback volume: no captured controls were available"
                        .to_string(),
                ));
            }

            let total_captured_volume_count = captured_volume_scalars.len();
            let partial_restore_error_message = match latest_restore_error {
                Some(partial_restore_error) => format!(
                    "Failed to restore macOS fallback volume on all captured controls ({successfully_restored_volume_count}/{total_captured_volume_count} restored). Last error: {partial_restore_error}"
                ),
                None => format!(
                    "Failed to restore macOS fallback volume on all captured controls ({successfully_restored_volume_count}/{total_captured_volume_count} restored)"
                ),
            };

            Err(AudioControlError::SetPropertyFailed(
                partial_restore_error_message,
            ))
        }
    }

    impl SystemAudioControl for MacOSAudioController {
        fn is_muted(&self) -> Result<bool, AudioControlError> {
            let current_default_output_device_id = Self::get_default_output_device()?;
            Self::get_u32_property(
                current_default_output_device_id,
                kAudioDevicePropertyMute,
                kAudioObjectPropertyElementMain,
            )
            .map(|raw_muted_value| raw_muted_value != 0)
        }

        fn begin_mute_session(&self) -> Result<ActiveMuteSession, AudioControlError> {
            let current_default_output_device_id = Self::get_default_output_device()?;
            match Self::select_mute_strategy_for_device(current_default_output_device_id) {
                MuteStrategy::DeviceMuteProperty => {
                    let initial_device_muted = Self::get_u32_property(
                        current_default_output_device_id,
                        kAudioDevicePropertyMute,
                        kAudioObjectPropertyElementMain,
                    )? != 0;

                    Self::set_u32_property(
                        current_default_output_device_id,
                        kAudioDevicePropertyMute,
                        kAudioObjectPropertyElementMain,
                        1,
                    )?;

                    Ok(ActiveMuteSession::MacOsDeviceMute {
                        output_device_id: current_default_output_device_id,
                        initial_device_muted,
                    })
                }
                MuteStrategy::VolumeZeroFallback => {
                    let captured_volume_scalars =
                        Self::apply_volume_zero_fallback_and_capture_snapshot(
                            current_default_output_device_id,
                        )?;

                    Ok(ActiveMuteSession::MacOsVolumeZeroFallback {
                        output_device_id: current_default_output_device_id,
                        captured_volume_scalars,
                    })
                }
            }
        }

        fn end_mute_session(
            &self,
            active_mute_session: &ActiveMuteSession,
        ) -> Result<(), AudioControlError> {
            match active_mute_session {
                ActiveMuteSession::MacOsDeviceMute {
                    output_device_id,
                    initial_device_muted,
                } => Self::restore_device_mute_session(*output_device_id, *initial_device_muted),
                ActiveMuteSession::MacOsVolumeZeroFallback {
                    output_device_id,
                    captured_volume_scalars,
                } => Self::restore_volume_zero_fallback_session(
                    *output_device_id,
                    captured_volume_scalars,
                ),
            }
        }
    }
}

/// The platform-free decisions of the macOS controller (tested everywhere).
mod pure {
    use super::{AudioControlError, OutputVolumeScalarSnapshot};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MuteStrategy {
        DeviceMuteProperty,
        VolumeZeroFallback,
    }

    pub enum VolumeZeroFallbackMuteAttemptResult {
        MutedAllSettable {
            captured_volume_scalars: Vec<OutputVolumeScalarSnapshot>,
        },
        FailedToMuteAllSettable {
            captured_volume_scalars: Vec<OutputVolumeScalarSnapshot>,
            total_settable_target_count: usize,
            mute_error: AudioControlError,
        },
    }

    fn build_partial_volume_zero_fallback_mute_error(
        successful_mute_count: usize,
        total_settable_target_count: usize,
        latest_volume_zero_error: Option<AudioControlError>,
    ) -> AudioControlError {
        let partial_mute_error_message = match latest_volume_zero_error {
            Some(partial_mute_error) => format!(
                "Failed to apply volume-zero fallback to all settable controls ({successful_mute_count}/{total_settable_target_count} muted). Last error: {partial_mute_error}"
            ),
            None => format!(
                "Failed to apply volume-zero fallback to all settable controls ({successful_mute_count}/{total_settable_target_count} muted)"
            ),
        };

        AudioControlError::SetPropertyFailed(partial_mute_error_message)
    }

    pub fn evaluate_volume_zero_fallback_mute_attempt(
        captured_volume_scalars: Vec<OutputVolumeScalarSnapshot>,
        total_settable_target_count: usize,
        latest_capture_error: Option<AudioControlError>,
        latest_apply_error: Option<AudioControlError>,
    ) -> VolumeZeroFallbackMuteAttemptResult {
        let successfully_muted_target_count = captured_volume_scalars.len();
        let latest_volume_zero_error = latest_apply_error.or(latest_capture_error);
        let muted_all_settable_targets_without_errors = latest_volume_zero_error.is_none()
            && successfully_muted_target_count != 0
            && successfully_muted_target_count == total_settable_target_count;

        if muted_all_settable_targets_without_errors {
            return VolumeZeroFallbackMuteAttemptResult::MutedAllSettable {
                captured_volume_scalars,
            };
        }

        if !captured_volume_scalars.is_empty() {
            let mute_error = build_partial_volume_zero_fallback_mute_error(
                successfully_muted_target_count,
                total_settable_target_count,
                latest_volume_zero_error,
            );
            return VolumeZeroFallbackMuteAttemptResult::FailedToMuteAllSettable {
                captured_volume_scalars,
                total_settable_target_count,
                mute_error,
            };
        }

        if total_settable_target_count == 0 {
            let mute_error = latest_volume_zero_error.unwrap_or_else(|| {
                AudioControlError::SetPropertyFailed(
                    "Failed to apply volume-zero fallback: no settable volume controls were available"
                        .to_string(),
                )
            });
            return VolumeZeroFallbackMuteAttemptResult::FailedToMuteAllSettable {
                captured_volume_scalars,
                total_settable_target_count,
                mute_error,
            };
        }

        let mute_error = build_partial_volume_zero_fallback_mute_error(
            successfully_muted_target_count,
            total_settable_target_count,
            latest_volume_zero_error,
        );
        VolumeZeroFallbackMuteAttemptResult::FailedToMuteAllSettable {
            captured_volume_scalars,
            total_settable_target_count,
            mute_error,
        }
    }

    pub fn select_mute_strategy(
        mute_property_exists: bool,
        mute_property_is_settable: bool,
    ) -> MuteStrategy {
        if mute_property_exists && mute_property_is_settable {
            MuteStrategy::DeviceMuteProperty
        } else {
            MuteStrategy::VolumeZeroFallback
        }
    }
}

#[cfg(test)]
#[path = "mute_tests.rs"]
mod tests;
