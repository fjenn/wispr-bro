// SPDX-License-Identifier: MIT AND GPL-3.0-or-later
// Copyright (c) 2026 handy-computer (handy-recorder 0.1.0, `backend/headset_macos.rs`)
// Modified by Wispr Bro contributors, 2026-10-31 (S-08).
//
// The CoreAudio property readers below (`address`, `ids`, `scalar`, `uid`,
// and the device list) are ported from handy-recorder's
// `src/backend/headset_macos.rs` (crates.io 0.1.0, upstream
// handy-computer/handy-recorder at 1d975fe8c10f9a69d778cf16955c7e0699b4b8e1,
// MIT), where they find a Bluetooth headset's output device. Changes: a
// string reader for the device name, an input-stream check, and the reads
// return one [`CoreDevice`] per device. The transport classification
// ([`Transport`], [`classify`]) and everything after it is Wispr Bro's own
// (GPL-3.0-or-later).
//! What kind of connection each audio device uses (R5 S-08).
//!
//! The microphone list marks the built-in mic as recommended and warns on
//! wireless ones ("Bluetooth can drop out"), and the default-mic rule
//! (`fork::audio::choice`) avoids a Bluetooth headset the user never chose.
//! Both need the CoreAudio transport type, which handy-recorder's device list
//! does not carry. The list is read straight from the HAL: a handful of
//! property reads, well under a millisecond, safe from any thread, and it
//! never opens a device (no microphone indicator).

use serde::{Deserialize, Serialize};
use specta::Type;

/// How a device is connected (CoreAudio `kAudioDevicePropertyTransportType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    BuiltIn,
    Usb,
    /// Classic Bluetooth and Bluetooth LE.
    Bluetooth,
    /// An iPhone used as a microphone over the air (Continuity).
    ContinuityWireless,
    /// An iPhone used as a microphone over a cable.
    ContinuityWired,
    AirPlay,
    Aggregate,
    Virtual,
    /// HDMI and DisplayPort (a display's speakers or mic).
    Display,
    Thunderbolt,
    Pci,
    FireWire,
    Avb,
    Unknown,
}

// The FourCC values, from CoreAudio `AudioHardware.h` (the same constants
// objc2-core-audio exports), so the classification is testable everywhere.
const BUILT_IN: u32 = u32::from_be_bytes(*b"bltn");
const USB: u32 = u32::from_be_bytes(*b"usb ");
const BLUETOOTH: u32 = u32::from_be_bytes(*b"blue");
const BLUETOOTH_LE: u32 = u32::from_be_bytes(*b"blea");
const CONTINUITY_WIRELESS: u32 = u32::from_be_bytes(*b"ccwl");
const CONTINUITY_WIRED: u32 = u32::from_be_bytes(*b"ccwd");
const CONTINUITY: u32 = u32::from_be_bytes(*b"ccap");
const AIRPLAY: u32 = u32::from_be_bytes(*b"airp");
const AGGREGATE: u32 = u32::from_be_bytes(*b"grup");
const AUTO_AGGREGATE: u32 = u32::from_be_bytes(*b"fgrp");
const VIRTUAL: u32 = u32::from_be_bytes(*b"virt");
const HDMI: u32 = u32::from_be_bytes(*b"hdmi");
const DISPLAY_PORT: u32 = u32::from_be_bytes(*b"dprt");
const THUNDERBOLT: u32 = u32::from_be_bytes(*b"thun");
const PCI: u32 = u32::from_be_bytes(*b"pci ");
const FIREWIRE: u32 = u32::from_be_bytes(*b"1394");
const AVB: u32 = u32::from_be_bytes(*b"eavb");

/// The transport for a raw `kAudioDevicePropertyTransportType` value.
pub fn classify(raw: u32) -> Transport {
    match raw {
        BUILT_IN => Transport::BuiltIn,
        USB => Transport::Usb,
        BLUETOOTH | BLUETOOTH_LE => Transport::Bluetooth,
        // The generic Continuity value has no wired/wireless hint; treat it
        // as wireless, the more cautious reading.
        CONTINUITY_WIRELESS | CONTINUITY => Transport::ContinuityWireless,
        CONTINUITY_WIRED => Transport::ContinuityWired,
        AIRPLAY => Transport::AirPlay,
        AGGREGATE | AUTO_AGGREGATE => Transport::Aggregate,
        VIRTUAL => Transport::Virtual,
        HDMI | DISPLAY_PORT => Transport::Display,
        THUNDERBOLT => Transport::Thunderbolt,
        PCI => Transport::Pci,
        FIREWIRE => Transport::FireWire,
        AVB => Transport::Avb,
        _ => Transport::Unknown,
    }
}

impl Transport {
    /// Connected over the air: can drop out mid-take, and a Bluetooth
    /// headset's mic switches it to its low-quality call profile.
    pub fn is_wireless(self) -> bool {
        matches!(
            self,
            Transport::Bluetooth | Transport::ContinuityWireless | Transport::AirPlay
        )
    }
}

/// One CoreAudio device, as the HAL lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreDevice {
    /// `kAudioDevicePropertyDeviceUID`: stable across replugs; cpal's device
    /// ID is `coreaudio:<uid>`.
    pub uid: String,
    pub name: String,
    pub transport: Transport,
    /// Has at least one input stream (a microphone).
    pub has_input: bool,
}

/// What the HAL says right now: every device, and the default input's UID.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub devices: Vec<CoreDevice>,
    pub default_input: Option<String>,
}

impl Snapshot {
    pub fn inputs(&self) -> impl Iterator<Item = &CoreDevice> {
        self.devices.iter().filter(|d| d.has_input)
    }

    pub fn find(&self, uid: &str) -> Option<&CoreDevice> {
        self.devices.iter().find(|d| d.uid == uid)
    }
}

/// handy-recorder's (cpal's) device ID for a CoreAudio UID.
pub fn recorder_id(uid: &str) -> String {
    format!("{CPAL_PREFIX}{uid}")
}

/// The CoreAudio UID inside a handy-recorder device ID, if it is one.
pub fn uid_of_recorder_id(id: &str) -> Option<&str> {
    id.strip_prefix(CPAL_PREFIX)
}

const CPAL_PREFIX: &str = "coreaudio:";

/// Reads the device list and the default input from the HAL. Empty off
/// macOS.
pub fn snapshot() -> Snapshot {
    #[cfg(target_os = "macos")]
    {
        hal::snapshot()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Snapshot::default()
    }
}

#[cfg(target_os = "macos")]
mod hal {
    use std::{
        ffi::c_void,
        ptr::{null, NonNull},
    };

    use objc2_core_audio::{
        kAudioDevicePropertyDeviceUID, kAudioDevicePropertyStreams,
        kAudioDevicePropertyTransportType, kAudioHardwarePropertyDefaultInputDevice,
        kAudioHardwarePropertyDevices, kAudioObjectPropertyElementMain, kAudioObjectPropertyName,
        kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyScopeInput, kAudioObjectSystemObject,
        AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectID,
        AudioObjectPropertyAddress, AudioObjectPropertyScope, AudioObjectPropertySelector,
    };
    use objc2_core_foundation::{CFRetained, CFString};

    use super::{classify, CoreDevice, Snapshot};

    pub fn snapshot() -> Snapshot {
        let system = kAudioObjectSystemObject as AudioObjectID;
        let default_input = scalar(
            system,
            kAudioHardwarePropertyDefaultInputDevice,
            kAudioObjectPropertyScopeGlobal,
        )
        .filter(|&id| id != 0)
        .and_then(|id| string(id, kAudioDevicePropertyDeviceUID));
        let devices = ids(
            system,
            kAudioHardwarePropertyDevices,
            kAudioObjectPropertyScopeGlobal,
        )
        .into_iter()
        .filter_map(|id| {
            Some(CoreDevice {
                uid: string(id, kAudioDevicePropertyDeviceUID)?,
                name: string(id, kAudioObjectPropertyName).unwrap_or_default(),
                transport: classify(
                    scalar(
                        id,
                        kAudioDevicePropertyTransportType,
                        kAudioObjectPropertyScopeGlobal,
                    )
                    .unwrap_or(0),
                ),
                has_input: !ids(
                    id,
                    kAudioDevicePropertyStreams,
                    kAudioObjectPropertyScopeInput,
                )
                .is_empty(),
            })
        })
        .collect();
        Snapshot {
            devices,
            default_input,
        }
    }

    fn address(
        selector: AudioObjectPropertySelector,
        scope: AudioObjectPropertyScope,
    ) -> AudioObjectPropertyAddress {
        AudioObjectPropertyAddress {
            mSelector: selector,
            mScope: scope,
            mElement: kAudioObjectPropertyElementMain,
        }
    }

    /// A property holding an array of object IDs.
    fn ids(
        object: AudioObjectID,
        selector: AudioObjectPropertySelector,
        scope: AudioObjectPropertyScope,
    ) -> Vec<AudioObjectID> {
        let address = address(selector, scope);
        let mut size = 0u32;
        // SAFETY: the address and size outlive the call.
        let status = unsafe {
            AudioObjectGetPropertyDataSize(
                object,
                NonNull::from(&address),
                0,
                null(),
                NonNull::from(&mut size),
            )
        };
        if status != 0 || size == 0 {
            return Vec::new();
        }
        let mut out = vec![0 as AudioObjectID; size as usize / size_of::<AudioObjectID>()];
        let mut size = (out.len() * size_of::<AudioObjectID>()) as u32;
        let Some(data) = NonNull::new(out.as_mut_ptr()) else {
            return Vec::new();
        };
        // SAFETY: `out` has room for `size` bytes, and CoreAudio writes at
        // most that many, updating `size` to what it wrote.
        let status = unsafe {
            AudioObjectGetPropertyData(
                object,
                NonNull::from(&address),
                0,
                null(),
                NonNull::from(&mut size),
                data.cast::<c_void>(),
            )
        };
        if status != 0 {
            return Vec::new();
        }
        out.truncate(size as usize / size_of::<AudioObjectID>());
        out
    }

    fn scalar(
        object: AudioObjectID,
        selector: AudioObjectPropertySelector,
        scope: AudioObjectPropertyScope,
    ) -> Option<u32> {
        let address = address(selector, scope);
        let mut value = 0u32;
        let mut size = size_of::<u32>() as u32;
        // SAFETY: `value` has room for the `size` bytes CoreAudio may write.
        let status = unsafe {
            AudioObjectGetPropertyData(
                object,
                NonNull::from(&address),
                0,
                null(),
                NonNull::from(&mut size),
                NonNull::from(&mut value).cast(),
            )
        };
        (status == 0 && size == size_of::<u32>() as u32).then_some(value)
    }

    /// A CFString property (UID, name).
    fn string(device: AudioObjectID, selector: AudioObjectPropertySelector) -> Option<String> {
        let address = address(selector, kAudioObjectPropertyScopeGlobal);
        let mut value: *mut CFString = std::ptr::null_mut();
        let mut size = size_of::<*mut CFString>() as u32;
        // SAFETY: both properties are a CFString pointer, returned under the
        // create rule (+1), which `CFRetained::from_raw` takes over.
        let status = unsafe {
            AudioObjectGetPropertyData(
                device,
                NonNull::from(&address),
                0,
                null(),
                NonNull::from(&mut size),
                NonNull::from(&mut value).cast(),
            )
        };
        if status != 0 {
            return None;
        }
        let value = NonNull::new(value)?;
        // SAFETY: see above; the pointer is non-null and owned.
        Some(unsafe { CFRetained::from_raw(value) }.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transports_classify_from_their_fourcc() {
        let cases = [
            (b"bltn", Transport::BuiltIn),
            (b"usb ", Transport::Usb),
            (b"blue", Transport::Bluetooth),
            (b"blea", Transport::Bluetooth),
            (b"ccwl", Transport::ContinuityWireless),
            (b"ccap", Transport::ContinuityWireless),
            (b"ccwd", Transport::ContinuityWired),
            (b"airp", Transport::AirPlay),
            (b"grup", Transport::Aggregate),
            (b"fgrp", Transport::Aggregate),
            (b"virt", Transport::Virtual),
            (b"hdmi", Transport::Display),
            (b"dprt", Transport::Display),
            (b"thun", Transport::Thunderbolt),
            (b"pci ", Transport::Pci),
            (b"1394", Transport::FireWire),
            (b"eavb", Transport::Avb),
            (b"????", Transport::Unknown),
        ];
        for (fourcc, want) in cases {
            assert_eq!(classify(u32::from_be_bytes(*fourcc)), want, "{fourcc:?}");
        }
        assert_eq!(classify(0), Transport::Unknown);
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[allow(deprecated)] // the generic Continuity value is deprecated but still sent
    fn our_fourcc_values_match_the_sdk() {
        use objc2_core_audio as ca;
        assert_eq!(BUILT_IN, ca::kAudioDeviceTransportTypeBuiltIn);
        assert_eq!(USB, ca::kAudioDeviceTransportTypeUSB);
        assert_eq!(BLUETOOTH, ca::kAudioDeviceTransportTypeBluetooth);
        assert_eq!(BLUETOOTH_LE, ca::kAudioDeviceTransportTypeBluetoothLE);
        assert_eq!(
            CONTINUITY_WIRELESS,
            ca::kAudioDeviceTransportTypeContinuityCaptureWireless
        );
        assert_eq!(
            CONTINUITY_WIRED,
            ca::kAudioDeviceTransportTypeContinuityCaptureWired
        );
        assert_eq!(CONTINUITY, ca::kAudioDeviceTransportTypeContinuityCapture);
        assert_eq!(AIRPLAY, ca::kAudioDeviceTransportTypeAirPlay);
        assert_eq!(AGGREGATE, ca::kAudioDeviceTransportTypeAggregate);
        assert_eq!(VIRTUAL, ca::kAudioDeviceTransportTypeVirtual);
        assert_eq!(HDMI, ca::kAudioDeviceTransportTypeHDMI);
        assert_eq!(DISPLAY_PORT, ca::kAudioDeviceTransportTypeDisplayPort);
        assert_eq!(THUNDERBOLT, ca::kAudioDeviceTransportTypeThunderbolt);
        assert_eq!(PCI, ca::kAudioDeviceTransportTypePCI);
        assert_eq!(FIREWIRE, ca::kAudioDeviceTransportTypeFireWire);
        assert_eq!(AVB, ca::kAudioDeviceTransportTypeAVB);
    }

    #[test]
    fn only_over_the_air_transports_are_wireless() {
        let wireless: Vec<_> = [
            Transport::BuiltIn,
            Transport::Usb,
            Transport::Bluetooth,
            Transport::ContinuityWireless,
            Transport::ContinuityWired,
            Transport::AirPlay,
            Transport::Aggregate,
            Transport::Virtual,
            Transport::Display,
            Transport::Thunderbolt,
            Transport::Unknown,
        ]
        .into_iter()
        .filter(|t| t.is_wireless())
        .collect();
        assert_eq!(
            wireless,
            [
                Transport::Bluetooth,
                Transport::ContinuityWireless,
                Transport::AirPlay
            ]
        );
    }

    #[test]
    fn recorder_ids_wrap_the_uid() {
        assert_eq!(
            recorder_id("BuiltInMicrophoneDevice"),
            "coreaudio:BuiltInMicrophoneDevice"
        );
        assert_eq!(
            uid_of_recorder_id("coreaudio:AppleUSBAudioEngine:1"),
            Some("AppleUSBAudioEngine:1")
        );
        assert_eq!(uid_of_recorder_id("USB Mic#1"), None);
    }

    /// Reads the real HAL: never opens a device. On a Mac with a mic the
    /// default input is one of the listed inputs.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_real_hal_lists_the_default_input_among_the_inputs() {
        let s = snapshot();
        if let Some(uid) = &s.default_input {
            assert!(s.inputs().any(|d| &d.uid == uid), "{s:?}");
        }
    }
}
