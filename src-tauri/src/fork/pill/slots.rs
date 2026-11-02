// SPDX-License-Identifier: GPL-3.0-or-later
//! Resting-bar slots (S-24): Dictate alone, or Dictate beside Scratchpad.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Dictate,
    Scratchpad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotSpec {
    pub slot: Slot,
    pub label_key: &'static str,
    pub tooltip_key: &'static str,
}

pub const DICTATE: SlotSpec = SlotSpec {
    slot: Slot::Dictate,
    label_key: "rust.pill.slots.dictate.label",
    tooltip_key: "rust.pill.slots.dictate.tooltip",
};
pub const SCRATCHPAD: SlotSpec = SlotSpec {
    slot: Slot::Scratchpad,
    label_key: "rust.pill.slots.scratchpad.label",
    tooltip_key: "rust.pill.slots.scratchpad.tooltip",
};

pub fn slots(scratchpad_enabled: bool) -> Vec<SlotSpec> {
    if scratchpad_enabled {
        vec![DICTATE, SCRATCHPAD]
    } else {
        vec![DICTATE]
    }
}

/// Horizontal hit target in the grouped resting bar. The boundary is stable
/// while a hover tooltip changes the panel's width.
pub fn hit_test(scratchpad_enabled: bool, x: f64, width: f64) -> Slot {
    if scratchpad_enabled && width > 0.0 && x >= width / 2.0 {
        Slot::Scratchpad
    } else {
        Slot::Dictate
    }
}

pub const GROUP_WIDTH: f64 = 184.0;
pub const GROUP_HEIGHT: f64 = 30.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_api_has_localized_labels_and_accessible_tooltips() {
        let keys = crate::fork::i18n::english_keys();
        assert_eq!(slots(false), vec![DICTATE]);
        assert_eq!(slots(true), vec![DICTATE, SCRATCHPAD]);
        for spec in slots(true) {
            assert!(
                keys.iter().any(|key| key == spec.label_key),
                "{}",
                spec.label_key
            );
            assert!(
                keys.iter().any(|key| key == spec.tooltip_key),
                "{}",
                spec.tooltip_key
            );
        }
        assert_eq!(hit_test(true, 20.0, 100.0), Slot::Dictate);
        assert_eq!(hit_test(true, 80.0, 100.0), Slot::Scratchpad);
        assert_eq!(hit_test(false, 80.0, 100.0), Slot::Dictate);
    }
}
