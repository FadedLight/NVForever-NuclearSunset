//! Why there is no result.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Failed {
    /// There is none for this input.
    Unavailable(Reason),
    /// The caller's check said so.
    Cancelled,
    /// A fault of the crate itself: a panic in it, or in the caller's check.
    Internal,
}

/// The caller's check: whether to stop. It is asked between pieces of work, on any thread.
pub type Cancel<'a> = &'a (dyn Fn() -> bool + Sync);

/// `f`, whose panics are [`Failed::Internal`].
pub fn guarded<T>(f: impl FnOnce() -> Result<T, Failed>) -> Result<T, Failed> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or(Err(Failed::Internal))
}

pub fn unavailable<T>(reason: Reason) -> Result<T, Failed> {
    Err(Failed::Unavailable(reason))
}

/// [`Failed::Cancelled`] when `cancel` says so.
pub fn go_on(cancel: Cancel) -> Result<(), Failed> {
    if cancel() {
        Err(Failed::Cancelled)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Reason {
    /// A number of the named input is not finite, or out of its range.
    Input(&'static str),
    /// More of the named than there may be.
    TooMany { what: &'static str, limit: usize },
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Reason::Input(what) => write!(f, "a number of the {what} is out of its range"),
            Reason::TooMany { what, limit } => write!(f, "more than {limit} {what}"),
        }
    }
}

impl fmt::Display for Failed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Failed::Unavailable(reason) => write!(f, "unavailable: {reason}"),
            Failed::Cancelled => write!(f, "cancelled"),
            Failed::Internal => write!(f, "a fault in the crate"),
        }
    }
}

impl std::error::Error for Failed {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panics_are_failures() {
        assert_eq!(guarded::<()>(|| panic!("expected here")), Err(Failed::Internal));
        assert_eq!(guarded(|| Ok(3)), Ok(3));
        assert!(guarded(|| go_on(&|| true)) == Err(Failed::Cancelled) && go_on(&|| false).is_ok());
    }
}
