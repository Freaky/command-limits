use std::ffi::OsStr;
use std::num::NonZeroUsize;
use std::os::windows::ffi::OsStrExt;

// Reserve a little, just in case.
const ARG_RESERVED: usize = 4096;

// Windows has seperate storage areas for arguments and environment, but
// they both share the same limit
const ARG_MAX: usize = 32767 - ARG_RESERVED;

pub(crate) fn osstr_len<S: AsRef<OsStr>>(s: S) -> usize {
    s.as_ref().encode_wide().count()
}

// Command line arguments are passed as a single contiguous string with elements
// quoted and escaped.
//
// Estimate how big the resulting string will be by double-counting backslashes
// and quotes, and assume quotes either side followed by a space or null
pub(crate) fn arg_len<S: AsRef<OsStr>>(arg: S) -> usize {
    arg.as_ref()
        .encode_wide()
        .map(|ch| {
            if ch == b'\\' as u16 || ch == b'"' as u16 {
                2
            } else {
                1
            }
        })
        .sum::<usize>()
        + 3
}

// Windows stores the environment as a null-delimited list of strings, which is
// itself null delimited.  We don't include the ending null for simplicity.
pub(crate) fn env_pair_len(k: &OsStr, v: &OsStr) -> usize {
    env_key_len(k) + env_val_len(v)
}

pub(crate) fn env_key_len(k: &OsStr) -> usize {
    osstr_len(k) + 1
}

pub(crate) fn env_val_len(k: &OsStr) -> usize {
    osstr_len(k) + 1
}

impl Default for crate::CommandLimits {
    fn default() -> Self {
        Self {
            arg_size: NonZeroUsize::new(ARG_MAX).unwrap(),
            individual_arg_size: None,
            arg_count: None,
            env_size: NonZeroUsize::new(ARG_MAX),
            individual_env_size: None,
            env_count: None,
        }
    }
}
