use crate::runners::{ProcessOutput, create_test_result};
use crate::test::TestResult;
use crate::{Error, TestStatus};
use mio::Token;
use std::collections::HashMap;
use std::io::BufReader;
use std::io::{BufRead, Read};
use std::time::Instant;

const STDOUT_TOKEN: Token = Token(0);
const STDERR_TOKEN: Token = Token(1);
const CHILD_TOKEN: Token = Token(2);

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
    PanicLocation {
        test: String,
        file: String,
        line: u32,
        column: u32,
    },
    PanicMessage {
        message: String,
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
        self.format_error()
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

    pub fn format_error(&self) -> String {
        match (&self.location, &self.message) {
            (Some(loc), Some(msg)) => {
                format!(
                    "--> {}:{}:{}\n    |\n    | {}\n    |",
                    loc.file, loc.line, loc.column, msg
                )
            }
            (Some(loc), None) => {
                format!("--> {}:{}:{}", loc.file, loc.line, loc.column)
            }
            (None, Some(msg)) => msg.clone(),
            (None, None) => "Unknown error".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TestRunState {
    tests: HashMap<String, TestState>,
    pending_errors: HashMap<String, ErrorInfo>,
    current_suite: Option<String>,
}

impl TestRunState {
    pub fn new() -> Self {
        Self {
            tests: HashMap::new(),
            pending_errors: HashMap::new(),
            current_suite: None,
        }
    }

    pub fn handle_line(&mut self, line: TestOutputLine) -> Option<ProcessOutput> {
        match line {
            TestOutputLine::TestStart { name } => {
                self.tests.insert(
                    name.clone(),
                    TestState::Running {
                        started_at: Instant::now(),
                    },
                );
                Some(ProcessOutput::TestStarted {
                    name,
                    suite: self.current_suite.clone().unwrap_or_default(),
                })
            }
            TestOutputLine::TestResult { name, status, .. } => {
                if let Some(TestState::Running { started_at }) = self.tests.get(&name) {
                    let duration_ms = started_at.elapsed().as_millis() as f64;
                    let error = self.pending_errors.remove(&name);

                    self.tests.insert(
                        name.clone(),
                        TestState::Completed {
                            duration_ms,
                            status: status.clone(),
                        },
                    );

                    match status {
                        TestStatus::Failed => Some(ProcessOutput::TestFailed {
                            result: create_test_result(&name, status),
                            duration_ms,
                            error: error.map(|e| e.to_string()).unwrap_or_default(),
                        }),
                        TestStatus::Passed => Some(ProcessOutput::TestPassed {
                            result: create_test_result(&name, status),
                            duration_ms,
                        }),
                        _ => Some(ProcessOutput::TestSkipped {
                            result: create_test_result(&name, status),
                        }),
                    }
                } else {
                    None
                }
            }
            TestOutputLine::PanicLocation {
                test,
                file,
                line,
                column,
            } => {
                self.pending_errors
                    .entry(test)
                    .or_insert_with(ErrorInfo::new)
                    .set_location(file, line, column);
                None
            }
            TestOutputLine::PanicMessage { message } => {
                if let Some((_, error)) = self.pending_errors.iter_mut().last() {
                    error.set_message(message);
                }
                None
            }
            _ => None,
        }
    }

    pub fn finalize_pending_errors(&mut self, test_results: &mut [TestResult]) {
        for result in test_results.iter_mut() {
            if let Some(error_info) = self.pending_errors.remove(&result.name) {
                if result.error.is_none() {
                    result.error = Some(Error::test_execution(error_info.format_error()));
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct LineBuffer<R: Read> {
    reader: BufReader<R>,
    buffer: Vec<u8>,
    partial: String,
}

impl<R: Read> LineBuffer<R> {
    pub fn new(reader: R) -> Self {
        let reader = BufReader::new(reader);
        Self {
            reader,
            buffer: vec![0; 4096],
            partial: String::new(),
        }
    }

    pub fn read_line(&mut self) -> std::io::Result<Option<String>> {
        loop {
            if let Some(pos) = self.partial.find('\n') {
                let line = self.partial[..pos].to_string();
                self.partial.drain(..=pos);
                return Ok(Some(line));
            }

            match self.reader.read(&mut self.buffer) {
                Ok(0) => {
                    if !self.partial.is_empty() {
                        let line = self.partial.clone();
                        self.partial.clear();
                        return Ok(Some(line));
                    }
                    return Ok(None);
                }
                Ok(n) => {
                    self.partial
                        .push_str(&String::from_utf8_lossy(&self.buffer[..n]));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return Ok(None);
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub fn flush_remaining(&mut self) -> Option<String> {
        if !self.partial.is_empty() {
            let line = self.partial.clone();
            self.partial.clear();
            Some(line)
        } else {
            None
        }
    }
}
