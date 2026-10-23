//! The engine's error type and its mapping onto the Python CLI's exit codes
//! and message prefixes.

use std::fmt;
use std::io;

#[derive(Debug)]
pub enum EngineError {
    /// `SandboxError` in the Python harness: exit code 3.
    Sandbox(String),
    /// `ProviderError`: exit code 2.
    Provider(String),
    /// `VerifierInfrastructureError`: exit code 3.
    VerifierInfrastructure(String),
    /// `ValueError`, `FileNotFoundError` and `BlockedEffortError`: exit code 1.
    Invalid(String),
    /// An I/O failure, with what the engine was doing at the time.
    Io { context: String, source: io::Error },
}

impl EngineError {
    pub fn io(context: impl Into<String>, source: io::Error) -> Self {
        EngineError::Io {
            context: context.into(),
            source,
        }
    }

    /// The process exit code `vulcanbench run` uses for this failure.
    pub fn exit_code(&self) -> i32 {
        match self {
            EngineError::Sandbox(_) | EngineError::VerifierInfrastructure(_) => 3,
            EngineError::Provider(_) => 2,
            EngineError::Invalid(_) | EngineError::Io { .. } => 1,
        }
    }

    /// The prefix the Python CLI prints before the message.
    pub fn cli_prefix(&self) -> &'static str {
        match self {
            EngineError::Sandbox(_) => "sandbox error",
            EngineError::Provider(_) => "provider error",
            EngineError::VerifierInfrastructure(_) => "verifier infrastructure error",
            EngineError::Invalid(_) | EngineError::Io { .. } => "error",
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Sandbox(m)
            | EngineError::Provider(m)
            | EngineError::VerifierInfrastructure(m)
            | EngineError::Invalid(m) => f.write_str(m),
            EngineError::Io { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EngineError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
