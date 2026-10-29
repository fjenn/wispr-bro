// SPDX-License-Identifier: GPL-3.0-or-later
//! The "Press 🌐 key to" setting: `defaults read com.apple.HIToolbox
//! AppleFnUsageType`. Read only; RS-01 never changes it (the owner does, in
//! System Settings). Value mapping per R4b §macOS (nix-darwin documents the
//! key): 0 Do Nothing, 1 Change Input Source, 2 Show Emoji & Symbols,
//! 3 Start Dictation. The session asks the owner to confirm what System
//! Settings shows for every value it reads, so the run verifies the mapping.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FnUsage {
    DoNothing,
    ChangeInputSource,
    ShowEmoji,
    StartDictation,
}

impl FnUsage {
    pub const ALL: [FnUsage; 4] = [
        FnUsage::DoNothing,
        FnUsage::ChangeInputSource,
        FnUsage::ShowEmoji,
        FnUsage::StartDictation,
    ];

    pub fn from_raw(v: i64) -> Option<FnUsage> {
        match v {
            0 => Some(FnUsage::DoNothing),
            1 => Some(FnUsage::ChangeInputSource),
            2 => Some(FnUsage::ShowEmoji),
            3 => Some(FnUsage::StartDictation),
            _ => None,
        }
    }

    /// The menu item's wording in System Settings → Keyboard.
    pub fn label(self) -> &'static str {
        match self {
            FnUsage::DoNothing => "Do Nothing",
            FnUsage::ChangeInputSource => "Change Input Source",
            FnUsage::ShowEmoji => "Show Emoji & Symbols",
            FnUsage::StartDictation => "Start Dictation",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GlobeSetting {
    /// The key does not exist: the Mac uses its default, never changed.
    NotSet,
    Value {
        raw: i64,
        usage: FnUsage,
    },
    /// A value outside 0..=3, or output we could not parse.
    Unknown {
        raw: String,
    },
}

impl GlobeSetting {
    pub fn usage(&self) -> Option<FnUsage> {
        match self {
            GlobeSetting::Value { usage, .. } => Some(*usage),
            _ => None,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            GlobeSetting::NotSet => "not set (never changed; the Mac uses its default)".to_string(),
            GlobeSetting::Value { raw, usage } => format!("\"{}\" (value {raw})", usage.label()),
            GlobeSetting::Unknown { raw } => format!("an unexpected value ({raw})"),
        }
    }
}

/// Parses the result of `defaults read com.apple.HIToolbox AppleFnUsageType`.
pub fn parse(success: bool, stdout: &str, stderr: &str) -> GlobeSetting {
    if !success {
        if stderr.contains("does not exist") {
            return GlobeSetting::NotSet;
        }
        return GlobeSetting::Unknown {
            raw: stderr.trim().to_string(),
        };
    }
    let text = stdout.trim();
    match text.parse::<i64>() {
        Ok(raw) => match FnUsage::from_raw(raw) {
            Some(usage) => GlobeSetting::Value { raw, usage },
            None => GlobeSetting::Unknown {
                raw: text.to_string(),
            },
        },
        Err(_) => GlobeSetting::Unknown {
            raw: text.to_string(),
        },
    }
}

/// Reads the current setting (spawns `defaults`; no network, no writes).
pub fn read() -> GlobeSetting {
    match std::process::Command::new("/usr/bin/defaults")
        .args(["read", "com.apple.HIToolbox", "AppleFnUsageType"])
        .output()
    {
        Ok(out) => parse(
            out.status.success(),
            &String::from_utf8_lossy(&out.stdout),
            &String::from_utf8_lossy(&out.stderr),
        ),
        Err(e) => GlobeSetting::Unknown {
            raw: format!("could not run defaults: {e}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_all_four_values() {
        for (raw, usage) in [
            ("0\n", FnUsage::DoNothing),
            ("1\n", FnUsage::ChangeInputSource),
            ("2\n", FnUsage::ShowEmoji),
            ("3\n", FnUsage::StartDictation),
        ] {
            let got = parse(true, raw, "");
            assert_eq!(got.usage(), Some(usage), "{raw}");
        }
    }

    #[test]
    fn missing_key_is_not_set() {
        let stderr = "2026-10-29 19:10:58.771 defaults[86469:39985611] \nThe domain/default pair of (com.apple.HIToolbox, AppleFnUsageType) does not exist\n";
        assert_eq!(parse(false, "", stderr), GlobeSetting::NotSet);
    }

    #[test]
    fn odd_values_are_unknown() {
        assert!(matches!(parse(true, "7", ""), GlobeSetting::Unknown { .. }));
        assert!(matches!(
            parse(true, "abc", ""),
            GlobeSetting::Unknown { .. }
        ));
        assert!(matches!(
            parse(false, "", "boom"),
            GlobeSetting::Unknown { .. }
        ));
    }

    #[test]
    fn labels_are_distinct() {
        let mut labels: Vec<_> = FnUsage::ALL.iter().map(|u| u.label()).collect();
        labels.dedup();
        assert_eq!(labels.len(), 4);
    }
}
