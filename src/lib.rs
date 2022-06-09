use std::ffi::OsString;
use std::num::NonZeroUsize;
use std::process::Command;
use std::collections::BTreeMap;
use std::{env, ffi::OsStr};

#[cfg_attr(unix, path = "unix.rs")]
#[cfg_attr(windows, path = "windows.rs")]
mod imp;

use imp::{arg_len, env_pair_len, env_val_len};

mod error;
pub use error::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Copy, Clone)]
pub struct CommandLimits {
    /// The maximum byte/character length for command arguments.
    pub arg_size: NonZeroUsize,
    /// The maximum size of an individual command-line argument.
    pub individual_arg_size: Option<NonZeroUsize>,
    /// The total number of arguments that are permitted.
    pub arg_count: Option<NonZeroUsize>,
    /// The maximum byte/character length for a command's environment variables.
    pub env_size: Option<NonZeroUsize>,
    /// The maximum byte/character length for individual key=value pairs in the
    /// environment.
    pub individual_env_size: Option<NonZeroUsize>,
    /// The maximum number of key=value pairs allowed in the environment.
    pub env_count: Option<NonZeroUsize>,
}

#[derive(Debug, Clone)]
pub struct CommandBuilder {
    limits: CommandLimits,
    argv: Vec<OsString>,
    env: BTreeMap<OsString, Option<OsString>>,
    arg_size: usize,
    env_size: usize,
    clear_env: bool,
}

impl CommandBuilder {
    /// Create a new `CommandBuilder` for the given `command` and inheriting the
    /// environment.
    ///
    /// Note the environment is assumed to be static, but not copied - modifications
    /// via `std::env::var_set` or similar will invalidate assumptions about the
    /// size of the environment, though small changes should not create significant
    /// difficulty.
    ///
    /// See `new_capture_env` for a version which copies the current environment
    /// to avoid this.
    pub fn new<S>(command: S) -> Result<Self>
    where
        S: AsRef<OsStr>,
    {
        Self::with_limits(command, Default::default())
    }

    /// Create a new `CommandBuilder` for the given `command` and current environment
    ///
    /// This function captures the environment so future changes to it cannot invalidate
    /// command size estimates, at the cost of additional storage and allocation.
    pub fn new_capture_env<S>(command: S) -> Result<Self>
    where
        S: AsRef<OsStr>,
    {
        Self::capture_with_limits(command, Default::default())
    }

    /// Create a new `CommandBuilder` with specified limits.
    pub fn with_limits<S>(command: S, limits: CommandLimits) -> Result<Self>
    where
        S: AsRef<OsStr>,
    {
        let mut cmd = Self {
            limits,
            argv: Default::default(),
            env: Default::default(),
            arg_size: Default::default(),
            env_size: Default::default(),
            clear_env: Default::default(),
        };

        cmd.inherit_env()?;
        cmd.arg(command)?;
        Ok(cmd)
    }

    /// Create a new `CommandBuilder` with specified limits and capturing the environment.
    pub fn capture_with_limits<S>(command: S, limits: CommandLimits) -> Result<Self>
    where
        S: AsRef<OsStr>,
    {
        let mut cmd = Self {
            limits,
            argv: Default::default(),
            env: Default::default(),
            arg_size: Default::default(),
            env_size: Default::default(),
            clear_env: Default::default(),
        };

        cmd.capture_env()?;
        cmd.arg(command)?;
        Ok(cmd)
    }

    /// Clear any previous env settings and restore the default behaviour of inheriting
    /// the current environment on spawn().
    ///
    /// If the environment would be too large to fit, it returns `Err`.
    pub fn inherit_env(&mut self) -> Result<&mut Self> {
        let old_env_size = self.env_size;
        self.env_size = env::vars_os().map(|(k,v)| env_pair_len(&k, &v)).sum();

        if let Err(e) = self.check_env_size(0) {
            self.env_size = old_env_size;
            return Err(e);
        }

        self.clear_env = false;
        self.env.clear();
        Ok(self)
    }

    /// Capture the environment into this `CommandBuilder` so that future modifications
    /// such as via `os::env::set_var` do not invalidate the expected environment
    /// size.
    ///
    /// This clears any previously set or removed env variables for this instance.
    ///
    /// If the environment would be too large to fit, it returns `Err`.
    pub fn capture_env(&mut self) -> Result<&mut Self> {
        let old_env_size = self.env_size;
        self.env_size = 0;

        let env: BTreeMap<OsString, Option<OsString>> = env::vars_os()
            .inspect(|(k, v)| self.env_size += env_pair_len(k, v))
            .map(|(k, v)| (k, Some(v)))
            .collect();

        if let Err(e) = self.check_env_size(0) {
            self.env_size = old_env_size;
            return Err(e);
        }

        self.clear_env = true;
        self.env = env;
        Ok(self)
    }

    /// Check the current command has space for `size` more environment data.
    fn check_env_size(&self, size: usize) -> Result<()> {
        // If the env limit is set, check against that
        if let Some(env_limit) = self.limits.env_size {
            if env_limit.get() < self.env_size + size {
                return Err(Error::InsufficientSpace);
            }
        } else if self.limits.arg_size.get() < self.arg_size + self.env_size + size {
            return Err(Error::InsufficientSpace);
        }

        Ok(())
    }

    fn check_env_pair(&self, key: &OsStr, val: &OsStr) -> Result<usize> {
        let len = env_pair_len(key, val);

        if self
            .limits
            .individual_env_size
            .or(self.limits.env_size)
            .unwrap_or(self.limits.arg_size)
            .get() < len
        {
            return Err(Error::TooLarge);
        }

        if self
            .limits
            .env_count
            .map(|limit| limit.get() <= self.env.len())
            .unwrap_or(false)
        {
            return Err(Error::TooMany);
        }

        self.check_env_size(len).map(|_| len)
    }

    /// Check if the given argument will accomodate our limits.
    ///
    /// Return an appropriate `Error` case or `Ok(size)` giving the number this
    /// would add to arg_size.
    fn check_arg(&self, arg: &OsStr) -> Result<usize> {
        let len = arg_len(arg);

        if self
            .limits
            .individual_arg_size
            .unwrap_or(self.limits.arg_size)
            .get() < len
        {
            return Err(Error::TooLarge);
        }

        if self
            .limits
            .arg_count
            .map(|limit| limit.get() <= self.argv.len())
            .unwrap_or(false)
        {
            return Err(Error::TooMany);
        }

        // if env and arg space is unified, we need to check both against arg_size
        if self.limits.env_size.is_some() {
            if self.limits.arg_size.get() < self.arg_size + len {
                return Err(Error::InsufficientSpace);
            }
        } else if self.limits.arg_size.get() < self.arg_size + self.env_size + len {
            return Err(Error::InsufficientSpace);
        }

        Ok(len)
    }

    /// Add the given argument to the command list if it fits.
    pub fn arg<S>(&mut self, arg: S) -> Result<&mut Self>
    where
        S: AsRef<OsStr>,
    {
        self.arg_size += self.check_arg(arg.as_ref())?;
        self.argv.push(arg.as_ref().to_owned());
        Ok(self)
    }

    /// Add the provided list of arguments to the command if they all fit.
    ///
    /// If the entire list does not fit, no arguments are added.
    pub fn args<S>(&mut self, args: &[S]) -> Result<&mut Self>
    where
        S: AsRef<OsStr>,
    {
        self.arg_size += args
            .iter()
            .map(|arg| self.check_arg(arg.as_ref()))
            .sum::<Result<usize>>()?;
        self.argv.extend(args.iter().map(|arg| arg.as_ref().to_owned()));
        Ok(self)
    }

    /// Set the given environment variable, if it will fit.
    pub fn env<K, V>(&mut self, key: K, value: V) -> Result<&mut Self>
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        if let Some(old_value) = self.env.get(key.as_ref())
        {
            // If it was previously set in the command, do we have space to exchange
            // the old value for the new one?
            if let Some(old_value) = old_value {
                let old_size = env_val_len(old_value);
                let new_size = env_val_len(value.as_ref());
                if old_size < new_size {
                    // TODO: check individual env size limit
                    self.check_env_size(new_size - old_size)?;
                }
                self.env_size = self.env_size.saturating_sub(old_size);
            }
        } else if let Some(old_value) = env::var_os(&key) {
            // Ditto if it instead exists in the inherited env and wasn't previously unset
            // FIXME: this needs a guard on self.clear_env
            let old_size = env_val_len(&old_value);
            let new_size = env_val_len(value.as_ref());
            if old_size < new_size {
                // TODO: check individual env size limit
                self.check_env_size(new_size - old_size)?;
            }
            self.env_size = self.env_size.saturating_sub(old_size);
        } else {
            // TODO: check env count limit
            self.env_size += self.check_env_pair(key.as_ref(), value.as_ref())?;
        }

        self.env.insert(key.as_ref().to_owned(), Some(value.as_ref().to_owned()));

        Ok(self)
    }

    /// Remove the given env variable
    ///
    /// This ensures the variable is not set even if it's added to the global environment later.
    pub fn env_remove<K>(&mut self, key: K) -> &mut Self
    where
        K: AsRef<OsStr>,
    {
        if let Some(value) = self.env.get(key.as_ref())
        {
            if let Some(value) = value {
                self.env_size = self.env_size.saturating_sub(env_pair_len(key.as_ref(), value));
            } else {
                // If it's already been set to None, do nothing instead of reinserting
                return self;
            }
        } else if !self.clear_env {
            if let Some(value) = env::var_os(key.as_ref()) {
                self.env_size = self.env_size.saturating_sub(env_pair_len(key.as_ref(), &value));
            }
        }

        if self.clear_env {
            self.env.remove(key.as_ref());
        } else {
            self.env.insert(key.as_ref().to_owned(), None);
        }

        self
    }

    /// Clear all env variables
    pub fn env_clear(&mut self) -> &mut Self {
        self.clear_env = true;
        self.env.clear();
        self.env_size = 0;
        self
    }

    // Create a `Command` from this `CommandBuilder`
    pub fn into_command(&self) -> Command {
        let mut cmd = Command::new(self.argv.get(0).expect("argv should not be empty"));
        if self.clear_env {
            cmd.env_clear();
        }

        for (k, v) in &self.env {
            if let Some(val) = v {
                cmd.env(k, val);
            } else {
                cmd.env_remove(k);
            }
        }

        if self.argv.len() > 1 {
            cmd.args(&self.argv[1..]);
        }

        cmd
    }
}

impl From<&CommandBuilder> for Command {
    fn from(builder: &CommandBuilder) -> Command {
        builder.into_command()
    }
}
