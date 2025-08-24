use std::time::Instant;

use mio::Token;
use strum_macros::{EnumDiscriminants, EnumString};

use crate::{TestStatus, runners::format_err_context, test::TestResult};

pub const STDOUT_TOKEN: Token = Token(0);
pub const STDERR_TOKEN: Token = Token(1);

#[derive(Debug, Clone, EnumDiscriminants)]
#[strum_discriminants(name(ProcessOutputType), derive(EnumString))]
pub enum ProcessOutput {
    #[strum(serialize = "test_started")]
    TestStarted {
        name: String,
        suite: String,
    },
    #[strum(serialize = "ok")]
    TestPassed {
        result: TestResult,
        duration_ms: f64,
    },
    #[strum(serialize = "FAILED")]
    TestFailed {
        result: TestResult,
        duration_ms: f64,
        error: String,
        location: Option<SourceLocation>,
    },
    #[strum(serialize = "test_skipped")]
    TestSkipped {
        result: TestResult,
    },
    #[strum(serialize = "running")]
    SuiteStarted {
        name: String,
        test_count: usize,
    },
    #[strum(serialize = "suite_completed")]
    SuiteCompleted {
        name: String,
    },
    Done,
}

#[derive(Debug, Clone)]
pub enum TestOutputLine {
    TestStart {
        name: String,
    },
    TestResult {
        name: String,
        status: TestStatus,
        duration_ms: Option<f64>,
    },
    SuiteStart {
        count: usize,
    },
    Panic {
        message: String,
        test: String,
        location: Option<SourceLocation>,
    },
}

#[derive(Debug, Clone)]
pub enum TestState {
    NotStarted,
    Running {
        started_at: Instant,
    },
    Completed {
        duration_ms: f64,
        status: TestStatus,
    },
}

#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub location: Option<SourceLocation>,
    pub message: Option<String>,
    pub backtrace: Vec<String>,
}

impl ToString for ErrorInfo {
    fn to_string(&self) -> String {
        format_err_context(
            "",
            self.location.clone(),
            self.message.as_ref().map(|m| m.as_str()),
        )
    }
}

impl ErrorInfo {
    pub fn new() -> Self {
        Self {
            location: None,
            message: None,
            backtrace: Vec::new(),
        }
    }

    pub fn set_location(&mut self, file: String, line: u32, column: u32) {
        self.location = Some(SourceLocation { file, line, column });
    }

    pub fn set_message(&mut self, message: String) {
        self.message = Some(message);
    }
}
