use serde::{Deserialize, Serialize};
use strum_macros::EnumDiscriminants;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

pub trait ResultExt<T> {
    fn into_test_error(self) -> Result<T>;
    fn into_fixture_error(self) -> Result<T>;
    fn into_hook_error<S: Into<String>>(self, hook_type: S) -> Result<T>;
    fn into_assertion_error(self) -> Result<T>;
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: std::fmt::Display,
{
    fn into_test_error(self) -> Result<T> {
        self.map_err(|e| Error::test_execution(e.to_string()))
    }

    fn into_fixture_error(self) -> Result<T> {
        self.map_err(|e| Error::fixture(e.to_string()))
    }

    fn into_hook_error<S: Into<String>>(self, hook_type: S) -> Result<T> {
        self.map_err(|e| Error::hook(hook_type.into(), e.to_string()))
    }

    fn into_assertion_error(self) -> Result<T> {
        self.map_err(|e| Error::assertion(e.to_string()))
    }

    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| Error::generic(format!("{}: {}", f(), e)))
    }
}

#[derive(Error, Debug, Clone, Serialize, Deserialize, EnumDiscriminants)]
#[strum_discriminants(name(ErrorKind), derive(Serialize, Deserialize))]
pub enum Error {
    #[error("Test execution failed: {message}")]
    TestExecution { message: String },

    #[error("Test setup failed: {message}")]
    TestSetup { message: String },

    #[error("Test teardown failed: {message}")]
    TestTeardown { message: String },

    #[error("Fixture error: {message}")]
    Fixture { message: String },

    #[error("Hook execution failed: {hook_type}: {message}")]
    Hook { hook_type: String, message: String },

    #[error("Assertion failed: {message}")]
    Assertion { message: String },

    #[error("Mock error: {message}")]
    Mock { message: String },

    #[error("Intended failure: {message}")]
    IntendedFailure { message: String },

    #[error("Runner configuration error: {message}")]
    RunnerConfig { message: String },

    #[error("Reporter error: {message}")]
    Reporter { message: String },

    #[error("Parameterization error: {message}")]
    Parameterization { message: String },

    #[error("Operation timed out: {message}")]
    Timeout { message: String },

    #[error("IO error: {message}")]
    Io { message: String },

    #[error("Serialization error: {message}")]
    Serialization { message: String },

    #[error("Invalid configuration: {message}")]
    InvalidConfig { message: String },

    #[error("{message}")]
    Generic { message: String },
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::TestExecution { .. } => ErrorKind::TestExecution,
            Error::TestSetup { .. } => ErrorKind::TestSetup,
            Error::TestTeardown { .. } => ErrorKind::TestTeardown,
            Error::Fixture { .. } => ErrorKind::Fixture,
            Error::Hook { .. } => ErrorKind::Hook,
            Error::Assertion { .. } => ErrorKind::Assertion,
            Error::Mock { .. } => ErrorKind::Mock,
            Error::IntendedFailure { .. } => ErrorKind::IntendedFailure,
            Error::RunnerConfig { .. } => ErrorKind::RunnerConfig,
            Error::Reporter { .. } => ErrorKind::Reporter,
            Error::Parameterization { .. } => ErrorKind::Parameterization,
            Error::Timeout { .. } => ErrorKind::Timeout,
            Error::Io { .. } => ErrorKind::Io,
            Error::Serialization { .. } => ErrorKind::Serialization,
            Error::InvalidConfig { .. } => ErrorKind::InvalidConfig,
            Error::Generic { .. } => ErrorKind::Generic,
        }
    }

    pub fn test_execution<S: Into<String>>(message: S) -> Self {
        Error::TestExecution {
            message: message.into(),
        }
    }

    pub fn intended_failure<S: Into<String>>(message: S) -> Self {
        Error::IntendedFailure {
            message: message.into(),
        }
    }

    pub fn test_setup<S: Into<String>>(message: S) -> Self {
        Error::TestSetup {
            message: message.into(),
        }
    }

    pub fn test_teardown<S: Into<String>>(message: S) -> Self {
        Error::TestTeardown {
            message: message.into(),
        }
    }

    pub fn fixture<S: Into<String>>(message: S) -> Self {
        Error::Fixture {
            message: message.into(),
        }
    }

    pub fn hook<S: Into<String>>(hook_type: S, message: S) -> Self {
        Error::Hook {
            hook_type: hook_type.into(),
            message: message.into(),
        }
    }

    pub fn assertion<S: Into<String>>(message: S) -> Self {
        Error::Assertion {
            message: message.into(),
        }
    }

    pub fn mock<S: Into<String>>(message: S) -> Self {
        Error::Mock {
            message: message.into(),
        }
    }

    pub fn timeout<S: Into<String>>(message: S) -> Self {
        Error::Timeout {
            message: message.into(),
        }
    }

    pub fn generic<S: Into<String>>(message: S) -> Self {
        Error::Generic {
            message: message.into(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io {
            message: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization {
            message: err.to_string(),
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Generic {
            message: err.to_string(),
        }
    }
}
