// A conservative fallback implementation

use std::ffi::OsStr;

const ARG_MAX: usize = 4096;

fn osstr_len<S: AsRef<OsStr>>(arg: S) -> usize {
    arg.as_ref().len() + 1
}

pub(crate) fn arg_len<S: AsRef<OsStr>>(arg: S) -> usize {
    // char * {arg}\0
    mem::size_of::<*const c_char>() + osstr_len(arg) + 1
}

pub(crate) fn env_pair_len(k: &OsStr, v: &OsStr) -> usize {
    // char * {k}={v}\0
    env_key_len(k) + env_val_len(v)
}

pub(crate) fn env_key_len(k: &OsStr) -> usize {
    mem::size_of::<*const c_char>() + osstr_len(k) + 1
}

pub(crate) fn env_val_len(v: &OsStr) -> usize {
    osstr_len(v) + 1
}

impl Default for crate::CommandLimits {
    fn default() -> Self {
        Self {
            arg_size: NonZeroUsize::new(ARG_MAX).unwrap(),
            individual_arg_size: None,
            arg_count: None,
            env_size: None,
            individual_env_size: None,
            env_count: None,
        }
    }
}
