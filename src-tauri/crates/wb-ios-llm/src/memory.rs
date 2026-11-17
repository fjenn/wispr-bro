// SPDX-License-Identifier: GPL-3.0-or-later
//! Memory facts for the iPhone memory rule: how much the device has and how
//! much the app uses (the number iOS's jetsam limit is measured against).

/// Installed RAM in bytes (`hw.memsize`).
pub fn physical_memory_bytes() -> u64 {
    #[cfg(target_vendor = "apple")]
    {
        let mut value: u64 = 0;
        let mut len = std::mem::size_of::<u64>();
        // SAFETY: a NUL-terminated name and an out buffer of `len` bytes.
        let rc = unsafe {
            sysctlbyname(
                c"hw.memsize".as_ptr(),
                (&mut value as *mut u64).cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc == 0 {
            return value;
        }
    }
    0
}

/// The process's physical footprint in bytes (`phys_footprint`, what Xcode's
/// memory gauge and jetsam use; includes Metal buffers), or `None`.
pub fn footprint_bytes() -> Option<u64> {
    #[cfg(target_vendor = "apple")]
    {
        // TASK_VM_INFO, counted in `natural_t` (u32) words. The buffer is
        // larger than any revision of `task_vm_info`; `phys_footprint` is the
        // u64 at byte offset 144 in every revision since rev1.
        const TASK_VM_INFO: u32 = 22;
        const PHYS_FOOTPRINT_OFFSET: usize = 144;
        let mut info = [0u32; 128];
        let mut count = info.len() as u32;
        // SAFETY: `info` is writable for `count` words.
        let rc = unsafe {
            task_info(
                mach_task_self_,
                TASK_VM_INFO,
                info.as_mut_ptr().cast(),
                &mut count,
            )
        };
        if rc != 0 || (count as usize) * 4 < PHYS_FOOTPRINT_OFFSET + 8 {
            return None;
        }
        let lo = u64::from(info[PHYS_FOOTPRINT_OFFSET / 4]);
        let hi = u64::from(info[PHYS_FOOTPRINT_OFFSET / 4 + 1]);
        return Some(lo | (hi << 32));
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(target_vendor = "apple")]
extern "C" {
    static mach_task_self_: u32;
    fn task_info(task: u32, flavor: u32, info: *mut i32, count: *mut u32) -> i32;
    fn sysctlbyname(
        name: *const std::os::raw::c_char,
        oldp: *mut std::os::raw::c_void,
        oldlenp: *mut usize,
        newp: *mut std::os::raw::c_void,
        newlen: usize,
    ) -> i32;
}
