use libc::{sysconf, _SC_ARG_MAX};

use std::ffi::OsStr;
use std::num::NonZeroUsize;
use std::os::unix::ffi::OsStrExt;

// POSIX guarantees at least 4k of space, but wants us to reserve at least 2k
// BSD prefers 4k, but if we were already at the floor go with POSIX
const ARG_POSIX_MIN: usize = 4096;
const ARG_RESERVED: usize = 4096;
const ARG_MIN: usize = 2048;

// _SC_ARG_MAX can be anything up to LONG_MAX, but let's not go too mad
const ARG_MAX: usize = 2048 * 1024;

// Linux limits individual argument strlen to 128k
#[cfg(target_os = "linux")]
const ARG_SINGLE_MAX: usize = 128 * 1024;

#[cfg(not(target_os = "linux"))]
const ARG_SINGLE_MAX: usize = 0;

// Assume 8 bytes, since 32-bit binaries may run on 64-bit operating systems,
// and thus inherit those limits.
const MAX_POINTER_SIZE: usize = 8;

fn _sc_arg_max() -> Option<usize> {
    let arg_max = unsafe { sysconf(_SC_ARG_MAX) };

    if arg_max > 0 {
        Some(arg_max as usize)
    } else {
        None
    }
}

pub(crate) fn osstr_len<S: AsRef<OsStr>>(s: S) -> usize {
    s.as_ref().as_bytes().len()
}

pub(crate) fn arg_len<S: AsRef<OsStr>>(arg: S) -> usize {
    // char * {arg}\0
    MAX_POINTER_SIZE + osstr_len(arg) + 1
}

pub(crate) fn env_pair_len(k: &OsStr, v: &OsStr) -> usize {
    env_key_len(k) + env_val_len(v)
}

pub(crate) fn env_key_len(k: &OsStr) -> usize {
    // char * {k}=
    MAX_POINTER_SIZE + osstr_len(k) + 1
}

pub(crate) fn env_val_len(v: &OsStr) -> usize {
    // {v}\0
    osstr_len(v) + 1
}

impl Default for crate::CommandLimits {
    fn default() -> Self {
        let arg_max = ARG_MAX
            .min(_sc_arg_max().unwrap_or_default())
            .max(ARG_POSIX_MIN)
            .saturating_sub(ARG_RESERVED)
            .max(ARG_MIN);

        Self {
            arg_size: NonZeroUsize::new(arg_max).unwrap(),
            individual_arg_size: NonZeroUsize::new(ARG_SINGLE_MAX),
            arg_count: None,
            env_size: None,
            individual_env_size: NonZeroUsize::new(ARG_SINGLE_MAX),
            env_count: None,
        }
    }
}
