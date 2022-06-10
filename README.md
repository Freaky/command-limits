# command_limits

Detect and enforce platform argument/environment size limits for command execution.

## Synopsis

```rust
pub struct CommandLimits {
    pub arg_size: NonZeroUsize,
    pub individual_arg_size: Option<NonZeroUsize>,
    pub arg_count: Option<NonZeroUsize>,
    pub env_size: Option<NonZeroUsize>,
    pub individual_env_size: Option<NonZeroUsize>,
    pub env_count: Option<NonZeroUsize>,
}

pub enum Error {
    InsufficientSpace,
    TooMany,
    TooLarge,
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct CommandBuilder { /* private */ }

impl CommandBuilder {
    pub fn new<S>(command: S) -> Result<Self>
    where
        S: AsRef<OsStr>;

    pub fn new_capture_env<S>(command: S) -> Result<Self>
    where
        S: AsRef<OsStr>;

    pub fn with_limits<S>(command: S, limits: CommandLimits) -> Result<Self>
    where
        S: AsRef<OsStr>;

    pub fn capture_with_limits<S>(command: S, limits: CommandLimits) -> Result<Self>
    where
        S: AsRef<OsStr>;

    pub fn arg<S>(&mut self, arg: S) -> Result<&mut Self>
    where
        S: AsRef<OsStr>;

    pub fn args<S>(&mut self, args: &[S]) -> Result<&mut Self>
    where
        S: AsRef<OsStr>;

    pub fn env<K, V>(&mut self, key: K, value: V) -> Result<&mut Self>
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>;

    pub fn env_remove<K>(&mut self, key: K) -> &mut Self
    where
        K: AsRef<OsStr>;

    pub fn inherit_env(&mut self) -> Result<&mut Self>;
    pub fn capture_env(&mut self) -> Result<&mut Self>;
    pub fn env_clear(&mut self) -> &mut Self;
    pub fn into_command(&self) -> std::process::Command;
}

impl From<&CommandBuilder> for std::process::Command;
```

## Description

`command_limits` provides a `CommandLimits` type specifying typical limits on the
size or number of command arguments and environment variables, and a `CommandBuilder`
which uses it to provide a fallible interface for specifying them.

This allows for the reliable creation of long command lines across different platforms,
without the need to shell out to `xargs(1)`.

## Example

Typical use is similar to that of `std::process::Command`, but with fallible methods
for specifying arguments and environment variables:

```rust
use command_limits::CommandBuilder;

let mut cmd = CommandBuilder::new("echo")?;
cmd.arg("hello, world")?.env("PATH", "/bin")?;
cmd.into_command().spawn()?;
```

By executing `arg()` or `args()` until `Error::TooMany` or `Error::InsufficientSpace`
is returned, an application can execute as long of a command as should be reasonably
expected to fit in the current environment.

`Error::TooLarge` indicates the argument or environment variable exceeds maximal limits
and cannot be specified even in principle.

Here we use `CommandBuilder` to echo the contents of a vec in as few calls as possible.

```rust
fn echo_vec(items: Vec<String>) -> Result<dyn Error> {
    let base = CommandBuilder::new("/bin/echo")?;
    let mut cmd = base.clone();

    for arg in items {
        match cmd.arg(arg) {
            Ok(_) => continue,
            Err(Error::TooLarge) => continue,
            Err(_) => {
                cmd.into_command().status()?;
                cmd = base.clone();
            }
        }
    }

    cmd.into_command().status()?;
    Ok(());
}
```
