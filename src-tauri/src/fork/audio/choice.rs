// SPDX-License-Identifier: GPL-3.0-or-later
//! Which microphone a take uses, and the order of the microphone list
//! (R5 S-08, A-37/A-38/A-39). Pure: works on a [`Snapshot`] of the HAL and
//! Handy's device names, so every rule has a unit test.
//!
//! The choice is Handy's `selected_microphone` (a device name, or `None` for
//! the system default) plus one fork setting, `mic_follow_system`, which
//! tells the two meanings of `None` apart:
//!
//! - **never chosen** (`None`, flag off): the system default, except that a
//!   wireless default (a Bluetooth headset, an iPhone over the air) is
//!   skipped for the built-in mic when the Mac has one. A headset that
//!   connects becomes the macOS default, and its mic drops the headset to
//!   its call profile and can cut out mid-take;
//! - **"Auto-detect (system setting)"** (`None`, flag on): always the system
//!   default, Bluetooth included: the user asked for it;
//! - **a device** (`Some(name)`): that device; Handy falls back to the
//!   default (and clears the name) when it is gone.
//!
//! List order: the built-in mic marked "(recommended)", "Auto-detect (system
//! setting)", then the other inputs, wired before wireless; wireless ones
//! carry a caution.

use serde::{Deserialize, Serialize};
use specta::Type;

use super::transport::{uid_of_recorder_id, CoreDevice, Snapshot, Transport};

/// The picker value of "Auto-detect (system setting)". Device entries use
/// the device name, as Handy stores it.
pub const AUTO: &str = "auto";

/// The built-in microphone, if the Mac has one (a Mac mini or Mac Studio
/// does not).
pub fn built_in(snapshot: &Snapshot) -> Option<&CoreDevice> {
    snapshot
        .inputs()
        .find(|d| d.transport == Transport::BuiltIn)
}

/// The default input, if the HAL lists it.
pub fn system_default(snapshot: &Snapshot) -> Option<&CoreDevice> {
    snapshot
        .default_input
        .as_deref()
        .and_then(|uid| snapshot.find(uid))
}

/// For a take that would open the system default (Handy's `device: None`):
/// the device to open instead, when the default is wireless, the user never
/// chose a mic and the Mac has a built-in one. `None` keeps the default.
pub fn avoid_wireless_default(snapshot: &Snapshot, follow_system: bool) -> Option<&CoreDevice> {
    if follow_system {
        return None;
    }
    let default = system_default(snapshot)?;
    if !default.transport.is_wireless() {
        return None;
    }
    built_in(snapshot).filter(|b| b.uid != default.uid)
}

/// The device a take opens for Handy's resolved `device` (a recorder ID, or
/// `None` for the default), after the wireless rule.
pub fn device_for<'a>(
    snapshot: &'a Snapshot,
    device: Option<&str>,
    follow_system: bool,
) -> Option<&'a CoreDevice> {
    match device {
        Some(id) => uid_of_recorder_id(id).and_then(|uid| snapshot.find(uid)),
        None => {
            avoid_wireless_default(snapshot, follow_system).or_else(|| system_default(snapshot))
        }
    }
}

/// Which device the current choice uses, by Handy's settings: `selected`
/// (a name) or the default with the wireless rule.
pub fn in_use<'a>(
    snapshot: &'a Snapshot,
    selected: Option<&str>,
    follow_system: bool,
) -> Option<&'a CoreDevice> {
    if let Some(name) = selected {
        if let Some(d) = snapshot.inputs().find(|d| d.name == name) {
            return Some(d);
        }
    }
    device_for(snapshot, None, follow_system)
}

/// What kind of row a microphone list entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum MicEntryKind {
    /// "Auto-detect (system setting)".
    Auto,
    Device,
}

/// One row of the microphone list (`fork_mic_list`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct MicEntry {
    /// What `fork_mic_choose` takes: [`AUTO`] or the device name.
    pub value: String,
    /// The device name (empty for Auto; the UI writes its own label).
    pub name: String,
    pub kind: MicEntryKind,
    /// The built-in mic: shown as "(recommended)".
    pub recommended: bool,
    /// Over the air: the list warns that it can drop out.
    pub wireless: bool,
    /// The macOS default input.
    pub system_default: bool,
    pub transport: Transport,
}

/// A device as handy-recorder lists it: its ID and name (Handy stores the
/// name).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecorderDevice {
    pub id: String,
    pub name: String,
}

/// The ordered list: built-in mic (recommended), Auto-detect, then the other
/// inputs (wired, then wireless). Devices come from handy-recorder (the
/// names Handy matches on); the transport from the HAL snapshot by UID.
pub fn list(devices: &[RecorderDevice], snapshot: &Snapshot) -> Vec<MicEntry> {
    let entry = |d: &RecorderDevice| {
        let core = uid_of_recorder_id(&d.id).and_then(|uid| snapshot.find(uid));
        let transport = core.map_or(Transport::Unknown, |c| c.transport);
        MicEntry {
            value: d.name.clone(),
            name: d.name.clone(),
            kind: MicEntryKind::Device,
            recommended: transport == Transport::BuiltIn,
            wireless: transport.is_wireless(),
            system_default: core
                .is_some_and(|c| snapshot.default_input.as_deref() == Some(c.uid.as_str())),
            transport,
        }
    };
    let mut rows: Vec<MicEntry> = Vec::with_capacity(devices.len() + 1);
    let all: Vec<MicEntry> = devices.iter().map(entry).collect();
    // One "(recommended)" row: the first built-in mic.
    let mut recommended_taken = false;
    let mut rest = Vec::new();
    for mut e in all {
        if e.recommended && !recommended_taken {
            recommended_taken = true;
            rows.push(e);
        } else {
            e.recommended = false;
            rest.push(e);
        }
    }
    rows.push(MicEntry {
        value: AUTO.into(),
        name: String::new(),
        kind: MicEntryKind::Auto,
        recommended: false,
        wireless: false,
        system_default: false,
        transport: Transport::Unknown,
    });
    // Stable: wired keep their order, then wireless keep theirs.
    rest.sort_by_key(|e| e.wireless);
    rows.extend(rest);
    rows
}

/// The picker value for Handy's `selected_microphone`.
pub fn selected_value(selected: Option<&str>) -> String {
    selected.map_or_else(|| AUTO.to_string(), str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(uid: &str, name: &str, transport: Transport) -> CoreDevice {
        CoreDevice {
            uid: uid.into(),
            name: name.into(),
            transport,
            has_input: true,
        }
    }

    fn output_only(uid: &str, name: &str, transport: Transport) -> CoreDevice {
        CoreDevice {
            has_input: false,
            ..dev(uid, name, transport)
        }
    }

    fn mac(default: &str, devices: Vec<CoreDevice>) -> Snapshot {
        Snapshot {
            devices,
            default_input: Some(default.into()),
        }
    }

    fn laptop_with_airpods(default: &str) -> Snapshot {
        mac(
            default,
            vec![
                dev(
                    "BuiltInMicrophoneDevice",
                    "MacBook Pro Microphone",
                    Transport::BuiltIn,
                ),
                output_only(
                    "BuiltInSpeakerDevice",
                    "MacBook Pro Speakers",
                    Transport::BuiltIn,
                ),
                dev("AA-BB:input", "AirPods Pro", Transport::Bluetooth),
                output_only("AA-BB:output", "AirPods Pro", Transport::Bluetooth),
                dev("AppleUSBAudioEngine:Shure", "Shure MV7", Transport::Usb),
            ],
        )
    }

    #[test]
    fn a_bluetooth_default_the_user_never_chose_uses_the_built_in_mic() {
        let s = laptop_with_airpods("AA-BB:input");
        let d = device_for(&s, None, false).unwrap();
        assert_eq!(d.uid, "BuiltInMicrophoneDevice");
        assert_eq!(
            avoid_wireless_default(&s, false).map(|d| d.name.as_str()),
            Some("MacBook Pro Microphone")
        );
    }

    #[test]
    fn auto_detect_chosen_follows_even_a_bluetooth_default() {
        let s = laptop_with_airpods("AA-BB:input");
        assert_eq!(device_for(&s, None, true).unwrap().uid, "AA-BB:input");
        assert!(avoid_wireless_default(&s, true).is_none());
    }

    #[test]
    fn a_wired_default_is_kept() {
        let s = laptop_with_airpods("AppleUSBAudioEngine:Shure");
        assert!(avoid_wireless_default(&s, false).is_none());
        assert_eq!(device_for(&s, None, false).unwrap().name, "Shure MV7");
        let s = laptop_with_airpods("BuiltInMicrophoneDevice");
        assert_eq!(
            device_for(&s, None, false).unwrap().name,
            "MacBook Pro Microphone"
        );
    }

    #[test]
    fn an_iphone_over_the_air_counts_as_wireless_but_a_cable_does_not() {
        let mut devices = vec![
            dev(
                "BuiltInMicrophoneDevice",
                "MacBook Air Microphone",
                Transport::BuiltIn,
            ),
            dev(
                "iphone",
                "Finn's iPhone Microphone",
                Transport::ContinuityWireless,
            ),
        ];
        let s = mac("iphone", devices.clone());
        assert_eq!(
            device_for(&s, None, false).unwrap().transport,
            Transport::BuiltIn
        );
        devices[1].transport = Transport::ContinuityWired;
        let s = mac("iphone", devices);
        assert_eq!(device_for(&s, None, false).unwrap().uid, "iphone");
    }

    #[test]
    fn a_mac_without_a_built_in_mic_keeps_the_bluetooth_default() {
        let s = mac(
            "AA-BB:input",
            vec![dev("AA-BB:input", "AirPods Pro", Transport::Bluetooth)],
        );
        assert!(avoid_wireless_default(&s, false).is_none());
        assert_eq!(device_for(&s, None, false).unwrap().uid, "AA-BB:input");
    }

    #[test]
    fn a_named_device_is_used_as_handy_resolved_it() {
        let s = laptop_with_airpods("BuiltInMicrophoneDevice");
        // Handy chose the AirPods by name: the rule never overrides a choice.
        let d = device_for(&s, Some("coreaudio:AA-BB:input"), false).unwrap();
        assert_eq!(d.name, "AirPods Pro");
        assert!(device_for(&s, Some("coreaudio:gone"), false).is_none());
        assert_eq!(
            in_use(&s, Some("Shure MV7"), false).unwrap().uid,
            "AppleUSBAudioEngine:Shure"
        );
        // A name that is gone: Handy falls back to the default (with the rule).
        let s = laptop_with_airpods("AA-BB:input");
        assert_eq!(
            in_use(&s, Some("Old USB mic"), false).unwrap().uid,
            "BuiltInMicrophoneDevice"
        );
    }

    #[test]
    fn no_default_input_means_nothing_to_open() {
        let s = Snapshot {
            devices: vec![],
            default_input: None,
        };
        assert!(device_for(&s, None, false).is_none());
    }

    fn recorder(uid: &str, name: &str) -> RecorderDevice {
        RecorderDevice {
            id: format!("coreaudio:{uid}"),
            name: name.into(),
        }
    }

    #[test]
    fn the_list_puts_built_in_then_auto_then_wired_then_wireless() {
        let s = laptop_with_airpods("AA-BB:input");
        // handy-recorder's order: AirPods first, as the HAL often lists them.
        let devices = [
            recorder("AA-BB:input", "AirPods Pro"),
            recorder("iphone", "Finn's iPhone Microphone"),
            recorder("AppleUSBAudioEngine:Shure", "Shure MV7"),
            recorder("BuiltInMicrophoneDevice", "MacBook Pro Microphone"),
        ];
        let rows = list(&devices, &s);
        let values: Vec<_> = rows.iter().map(|r| r.value.as_str()).collect();
        assert_eq!(
            values,
            [
                "MacBook Pro Microphone",
                AUTO,
                // The iPhone is not in this snapshot: unknown transport,
                // treated as wired (no caution we cannot back up).
                "Finn's iPhone Microphone",
                "Shure MV7",
                "AirPods Pro",
            ]
        );
        assert!(rows[0].recommended && !rows[0].wireless);
        assert_eq!(rows[1].kind, MicEntryKind::Auto);
        assert_eq!(rows[2].transport, Transport::Unknown);
        let airpods = rows.last().unwrap();
        assert!(airpods.wireless && airpods.system_default && !airpods.recommended);
        assert_eq!(rows.iter().filter(|r| r.recommended).count(), 1);
        assert_eq!(rows.iter().filter(|r| r.system_default).count(), 1);
    }

    #[test]
    fn a_mac_without_a_built_in_mic_starts_the_list_with_auto() {
        let s = mac(
            "AppleUSBAudioEngine:Shure",
            vec![dev(
                "AppleUSBAudioEngine:Shure",
                "Shure MV7",
                Transport::Usb,
            )],
        );
        let rows = list(&[recorder("AppleUSBAudioEngine:Shure", "Shure MV7")], &s);
        assert_eq!(rows[0].kind, MicEntryKind::Auto);
        assert!(!rows.iter().any(|r| r.recommended));
        assert!(rows[1].system_default);
    }

    #[test]
    fn the_picker_value_of_the_default_is_auto() {
        assert_eq!(selected_value(None), AUTO);
        assert_eq!(selected_value(Some("Shure MV7")), "Shure MV7");
    }
}
