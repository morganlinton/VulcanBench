//! Error reporting API.

use std::error::Error;
use std::fmt;

/// Result of a cargo command.
pub type CargoResult<T> = Result<T, CargoError>;

/// For programmatically processing failures.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// Spawning the cargo subommand failed.
    InvalidCommand,
    /// The cargo subcommand returned an error.
    CommandFailed,
    /// Parsing the cargo subcommand's output failed.
    InvalidOutput,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ErrorKind::InvalidOutput => write!(f, "Spawning the cargo subommand failed."),
            ErrorKind::CommandFailed => write!(f, "The cargo subcommand returned an error."),
            ErrorKind::InvalidCommand => write!(f, "Parsing the cargo subcommand's output failed."),
        }
    }
}

/// Cargo command failure information.
#[derive(Debug)]
pub struct CargoError {
    kind: ErrorKind,
    context: Option<String>,
    cause: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl CargoError {
    pub(crate) fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            context: None,
            cause: None,
        }
    }

    pub(crate) fn set_context<S>(mut self, context: S) -> Self
    where
        S: Into<String>,
    {
        let context = context.into();
        self.context = Some(context);
        self
    }

    pub(crate) fn set_cause<E>(mut self, cause: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        let cause = Box::new(cause);
        self.cause = Some(cause);
        self
    }

    /// For programmatically processing failures.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl Error for CargoError {
    fn cause(&self) -> Option<&dyn Error> {
        self.cause.as_ref().map(|c| {
            let c: &dyn Error = c.as_ref();
            c
        })
    }
}

impl fmt::Display for CargoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Cargo command failed: {}", self.kind)?;
        if let Some(ref context) = self.context {
            writeln!(f, "{context}")?;
        }
        if let Some(ref cause) = self.cause {
            writeln!(f, "Cause: {cause}")?;
        }
        Ok(())
    }
}
