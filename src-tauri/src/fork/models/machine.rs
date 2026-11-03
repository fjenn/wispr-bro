// SPDX-License-Identifier: GPL-3.0-or-later
//! What this Mac is, for the silent model policy (S-07b, DEC-02, B-49).
//!
//! sysinfo (R4: `system`, `disk`) reads the chip's brand string, the RAM and
//! the free space of the volume that holds the model cache. On macOS
//! sysinfo asks for `AvailableCapacityForImportantUsage`, which counts space
//! macOS can purge (caches, local snapshots) as free, the number Finder
//! shows. The probe runs off the main thread and only when the policy
//! re-picks (launch, a language change, a slow-speed verdict).

use std::path::Path;

use serde::{Deserialize, Serialize};
use specta::Type;

/// Apple Silicon tier from the brand string (`Apple M2 Pro`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Base,
    Pro,
    Max,
    Ultra,
}

/// The chip, as far as the policy cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Chip {
    /// `generation` 1 for M1, 5 for M5; `tier` from the suffix.
    AppleSilicon { generation: u8, tier: Tier },
    /// Apple Silicon whose brand string we could not read.
    UnknownAppleSilicon,
    /// x86_64 (DEC-29: CPU only, later milestone).
    Intel,
}

/// One probe of this Mac.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Machine {
    pub chip: Chip,
    /// Installed RAM in bytes.
    pub ram_bytes: u64,
    /// Free bytes on the model volume (purgeable-aware), when known.
    pub free_disk_bytes: Option<u64>,
}

impl Machine {
    /// RAM in whole GB as Apple sells it (8, 16, 18, 24, …).
    pub fn ram_gb(&self) -> u64 {
        (self.ram_bytes + (1 << 29)) >> 30
    }
}

/// Parses `machdep.cpu.brand_string` / sysinfo's CPU brand.
pub fn parse_chip(brand: &str, arch: &str) -> Chip {
    if arch == "x86_64" {
        return Chip::Intel;
    }
    let b = brand.trim();
    let Some(rest) = b
        .strip_prefix("Apple M")
        .or_else(|| b.strip_prefix("Apple m"))
    else {
        return Chip::UnknownAppleSilicon;
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    let Ok(generation) = digits.parse::<u8>() else {
        return Chip::UnknownAppleSilicon;
    };
    if generation == 0 {
        return Chip::UnknownAppleSilicon;
    }
    let suffix = rest[digits.len()..].trim().to_ascii_lowercase();
    let tier = if suffix.contains("ultra") {
        Tier::Ultra
    } else if suffix.contains("max") {
        Tier::Max
    } else if suffix.contains("pro") {
        Tier::Pro
    } else {
        Tier::Base
    };
    Chip::AppleSilicon { generation, tier }
}

/// The free space sysinfo reports for the volume holding `path`: the disk
/// whose mount point is the longest prefix of it.
pub fn free_space_for(path: &Path, disks: &[(std::path::PathBuf, u64)]) -> Option<u64> {
    disks
        .iter()
        .filter(|(mount, _)| path.starts_with(mount))
        .max_by_key(|(mount, _)| mount.as_os_str().len())
        .map(|(_, free)| *free)
}

/// Probes this Mac. `model_dir` is where speech models are stored (the
/// Hugging Face cache).
pub fn probe(model_dir: &Path) -> Machine {
    use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};
    let sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_memory(MemoryRefreshKind::nothing().with_ram())
            .with_cpu(CpuRefreshKind::nothing()),
    );
    let brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_default();
    let disks = Disks::new_with_refreshed_list();
    let list: Vec<(std::path::PathBuf, u64)> = disks
        .list()
        .iter()
        .map(|d| (d.mount_point().to_path_buf(), d.available_space()))
        .collect();
    let existing = model_dir
        .ancestors()
        .find(|p| p.exists())
        .unwrap_or(model_dir);
    let free =
        free_space_for(existing, &list).or_else(|| crate::fork::db::disk::free_bytes(existing));
    let machine = Machine {
        chip: parse_chip(&brand, std::env::consts::ARCH),
        // Debug builds: `WB_DEBUG_MEMORY` physical_gb simulates another
        // Mac's RAM tier (S-38, Mac test S-38.7).
        ram_bytes: crate::fork::llm::memory::simulated_physical_bytes()
            .unwrap_or_else(|| sys.total_memory()),
        free_disk_bytes: free,
    };
    log::info!(
        "models: this Mac: {:?}, {} GB RAM, {} GB free for models",
        machine.chip,
        machine.ram_gb(),
        machine
            .free_disk_bytes
            .map(|b| b / 1_000_000_000)
            .unwrap_or(0)
    );
    machine
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn brand_strings_parse() {
        let c = |b| parse_chip(b, "aarch64");
        assert_eq!(
            c("Apple M1"),
            Chip::AppleSilicon {
                generation: 1,
                tier: Tier::Base
            }
        );
        assert_eq!(
            c("Apple M2 Pro"),
            Chip::AppleSilicon {
                generation: 2,
                tier: Tier::Pro
            }
        );
        assert_eq!(
            c("Apple M5 Max"),
            Chip::AppleSilicon {
                generation: 5,
                tier: Tier::Max
            }
        );
        assert_eq!(
            c("Apple M1 Ultra"),
            Chip::AppleSilicon {
                generation: 1,
                tier: Tier::Ultra
            }
        );
        assert_eq!(
            c("Apple M12"),
            Chip::AppleSilicon {
                generation: 12,
                tier: Tier::Base
            }
        );
        assert_eq!(c(""), Chip::UnknownAppleSilicon);
        assert_eq!(c("Apple Virtual"), Chip::UnknownAppleSilicon);
        assert_eq!(c("Apple M0"), Chip::UnknownAppleSilicon);
        assert_eq!(
            parse_chip("Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz", "x86_64"),
            Chip::Intel
        );
    }

    #[test]
    fn ram_rounds_to_marketing_gb() {
        let m = |b| Machine {
            chip: Chip::Intel,
            ram_bytes: b,
            free_disk_bytes: None,
        };
        assert_eq!(m(8 << 30).ram_gb(), 8);
        assert_eq!(m(137_438_953_472).ram_gb(), 128);
        assert_eq!(m((16 << 30) - 50_000_000).ram_gb(), 16);
    }

    #[test]
    fn free_space_comes_from_the_deepest_mount() {
        let disks = vec![
            (PathBuf::from("/"), 10),
            (PathBuf::from("/System/Volumes/Data"), 500),
            (PathBuf::from("/Volumes/Ext"), 7),
        ];
        assert_eq!(
            free_space_for(Path::new("/System/Volumes/Data/Users/a"), &disks),
            Some(500)
        );
        assert_eq!(free_space_for(Path::new("/Volumes/Ext/x"), &disks), Some(7));
        assert_eq!(free_space_for(Path::new("/Users/a"), &disks), Some(10));
        assert_eq!(free_space_for(Path::new("/Users/a"), &[]), None);
    }

    #[test]
    fn the_probe_runs_on_this_mac() {
        let dir = std::env::temp_dir();
        let m = probe(&dir);
        assert!(m.ram_bytes > 1 << 30);
        assert!(m.free_disk_bytes.is_some());
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            assert_ne!(m.chip, Chip::Intel);
        }
    }
}
