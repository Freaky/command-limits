use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Error {
    /// The value(s) given are too long to fit in the remaining argument/environment
    /// space, either because they are too big or there are already too many arguments.
    ///
    /// It is possible they could fit into another, smaller invocation.
    InsufficientSpace,
    /// While there may be space for this value, the limit on the total number of
    /// arguments or environment variables would be exceeded.
    TooMany,
    /// The value(s) given exceed limits on individual arguments, and are not expected
    /// to work even if retried with a smaller command.
    TooLarge,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Error::TooMany => "too many values",
                Error::TooLarge => "value is too large",
                Error::InsufficientSpace => "insufficient space for value",
            }
        )
    }
}

impl std::error::Error for Error {}
