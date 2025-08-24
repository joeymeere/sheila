use std::{
    collections::HashMap,
    io::{BufReader, Read},
    path::PathBuf,
    time::Instant,
};

use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_until, take_while1},
    character::complete::digit1,
    combinator::map,
};
use uuid::Uuid;

use crate::{
    Error, ErrorInfo, ProcessOutput, Result, RunnerConfig, SourceLocation, TestMetadata,
    TestOutputLine, TestState, TestStatus,
    runners::{RunResult, format_mod_name},
    suite::SuiteResult,
    test::TestResult,
};

#[derive(Debug, Clone)]
pub struct TestTracker {
    pub current_test_name: Option<String>,
    pub previous_test_name: Option<String>,
    pub test_state: TestRunState,
    pub timer: Instant,
}

impl TestTracker {
    pub fn new() -> Self {
        Self {
            current_test_name: None,
            previous_test_name: None,
            timer: Instant::now(),
            test_state: TestRunState::new(),
        }
    }

    pub fn start_test(&mut self, test_name: String) {
        self.previous_test_name = self.current_test_name.clone();
        self.current_test_name = Some(test_name);
        self.timer = Instant::now();
    }

    pub fn end_test(&mut self) {
        self.previous_test_name = self.current_test_name.clone();
        self.current_test_name = None;
    }

    pub fn end_test_with_error(&mut self, err: String) {
        self.previous_test_name = self.current_test_name.clone();
        self.current_test_name = None;
        self.test_state.handle_line(TestOutputLine::Panic {
            message: err,
            test: self.current_test_name.clone().unwrap(),
            location: None,
        });
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.timer.elapsed().as_millis_f64() as f64
    }
}

impl Default for TestTracker {
    fn default() -> Self {
        Self::new()
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
                            result: StandardLineParser::create_test_result(&name, status),
                            duration_ms,
                            error: error.clone().map(|e| e.to_string()).unwrap_or_default(),
                            location: error.as_ref().and_then(|e| e.location.clone()),
                        }),
                        TestStatus::Passed => Some(ProcessOutput::TestPassed {
                            result: StandardLineParser::create_test_result(&name, status),
                            duration_ms,
                        }),
                        _ => Some(ProcessOutput::TestSkipped {
                            result: StandardLineParser::create_test_result(&name, status),
                        }),
                    }
                } else {
                    None
                }
            }
            TestOutputLine::Panic {
                message,
                test,
                location,
            } => {
                self.pending_errors
                    .entry(test.clone())
                    .or_insert_with(ErrorInfo::new)
                    .set_location(
                        location
                            .as_ref()
                            .map(|l| l.file.clone())
                            .unwrap_or_default(),
                        location.as_ref().map(|l| l.line).unwrap_or(0),
                        location.as_ref().map(|l| l.column).unwrap_or(0),
                    );

                self.pending_errors
                    .get_mut(&test)
                    .unwrap()
                    .set_message(message);

                None
            }
            _ => None,
        }
    }

    pub fn finalize_pending_errors(&mut self, test_results: &mut [TestResult]) {
        for result in test_results.iter_mut() {
            if let Some(error_info) = self.pending_errors.remove(&result.name) {
                if result.error.is_none() {
                    result.error = Some(Error::test_execution(error_info.to_string()));
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TestExecutable {
    pub path: PathBuf,
    pub name: String,
    pub package_name: String,
    pub target_crate: String,
}

impl TestExecutable {
    pub fn new(path: PathBuf, name: String, package_name: String) -> Self {
        let target_crate = Self::determine_target_crate(&path);
        Self {
            path,
            name,
            package_name,
            target_crate,
        }
    }

    // TODO: remove this -- fuckin hack
    pub fn determine_target_crate(path: &PathBuf) -> String {
        let path_str = path.to_string_lossy();
        if path_str.contains("examples") {
            "examples".to_string()
        } else if path_str.contains("cli") {
            "cli".to_string()
        } else if path_str.contains("core") {
            "core".to_string()
        } else if path_str.contains("server") {
            "server".to_string()
        } else if path_str.contains("proc-macros") || path_str.contains("proc_macros") {
            "proc_macros".to_string()
        } else {
            "examples".to_string()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StandardLineParser;

impl StandardLineParser {
    pub fn parse_test_output(input: &str) -> IResult<&str, TestOutputLine> {
        alt((
            Self::parse_test_result,
            Self::parse_test_start,
            Self::parse_suite_start,
        ))
        .parse(input)
    }

    pub fn parse_error_output(input: &str) -> Result<TestOutputLine> {
        let (_, result) =
            Self::parse_panic(input).map_err(|e| Error::test_execution(e.to_string()))?;

        Ok(result)
    }

    fn parse_test_start(input: &str) -> IResult<&str, TestOutputLine> {
        let (input, _) = tag("test ")(input)?;
        let (input, name) = take_while1(|c: char| !c.is_whitespace())(input)?;

        Ok((
            input,
            TestOutputLine::TestStart {
                name: name.to_string(),
            },
        ))
    }

    fn parse_suite_start(input: &str) -> IResult<&str, TestOutputLine> {
        let (input, _) = tag("running ")(input)?;
        let (input, count_str) = digit1(input)?;
        let count = count_str.parse::<usize>().unwrap_or(0);
        let (input, _) = tag(" test")(input)?;

        Ok((input, TestOutputLine::SuiteStart { count }))
    }

    fn parse_test_result(input: &str) -> IResult<&str, TestOutputLine> {
        let (input, _) = tag("test ")(input)?;
        let (input, name) = take_until(" ")(input)?;
        let (input, _) = tag(" ... ")(input)?;
        let (input, status) = alt((
            map(tag("ok"), |_| TestStatus::Passed),
            map(tag("FAILED"), |_| TestStatus::Failed),
            map(tag("ignored"), |_| TestStatus::Skipped),
        ))
        .parse(input)?;

        Ok((
            input,
            TestOutputLine::TestResult {
                name: name.to_string(),
                status,
                duration_ms: None,
            },
        ))
    }

    fn parse_panic(input: &str) -> IResult<&str, TestOutputLine> {
        let (input, _) = tag("thread '")(input)?;
        let (input, test_name) = take_until("'")(input)?;
        let (input, _) = tag("' panicked at ")(input)?;
        let (input, file) = take_until(":")(input)?;
        let (input, _) = tag(":")(input)?;
        let (input, line_str) = digit1(input)?;
        let line = line_str.parse::<u32>().unwrap();
        let (input, _) = tag(":")(input)?;
        let (input, column_str) = digit1(input)?;
        let column = column_str.parse::<u32>().unwrap();
        let (input, _) = tag(":\n")(input)?;
        let err_message = input;

        let full_err = err_message.trim().to_string();

        Ok((
            "",
            TestOutputLine::Panic {
                message: full_err,
                test: test_name.to_string(),
                location: Some(SourceLocation {
                    file: file.to_string(),
                    line,
                    column,
                }),
            },
        ))
    }

    pub fn create_test_result(name: &str, status: TestStatus) -> TestResult {
        let test_id = Uuid::new_v4();
        let name = format_mod_name(name);

        let metadata = TestMetadata::new(name.clone());
        let mut test_result = TestResult::new(test_id, name, metadata);

        match status {
            TestStatus::Passed => test_result.finish(TestStatus::Passed, None),
            TestStatus::Failed => test_result.finish(
                TestStatus::Failed,
                Some(Error::test_execution("Test failed")),
            ),
            _ => test_result.finish(TestStatus::Skipped, None),
        }

        test_result
    }

    pub fn create_suite_result(suite_name: &str, test_results: &[TestResult]) -> SuiteResult {
        let suite_id = Uuid::new_v4();
        let name = format_mod_name(suite_name);
        let metadata = TestMetadata::new(name.clone());
        let mut suite_result = SuiteResult::new(suite_id, name, metadata);

        for test_result in test_results {
            suite_result.add_test_result(test_result.clone());
        }

        suite_result
    }

    #[allow(unused)]
    pub fn create_run_result(suite_name: &str, test_results: &[TestResult]) -> RunResult {
        let name = format_mod_name(suite_name);
        let mut run_result = RunResult::new(RunnerConfig::default());

        let suite_result = Self::create_suite_result(&name, test_results);
        run_result.add_suite_result(suite_result);

        run_result
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct JsonLineParser;

impl JsonLineParser {
    pub fn parse_test_output(input: &str) -> Result<Option<TestOutputLine>> {
        let json: serde_json::Value = serde_json::from_str(input)?;

        let event_type = json.get("type").and_then(|v| v.as_str());
        let event = json.get("event").and_then(|v| v.as_str());

        match (event_type, event) {
            (Some("suite"), Some("started")) => {
                let test_count =
                    json.get("test_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

                Ok(Some(TestOutputLine::SuiteStart { count: test_count }))
            }
            (Some("test"), Some("started")) => {
                if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                    Ok(Some(TestOutputLine::TestStart {
                        name: name.to_string(),
                    }))
                } else {
                    Ok(None)
                }
            }
            (Some("test"), Some("ok")) => {
                if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                    let duration_ms = json.get("exec_time").and_then(|v| v.as_f64());

                    Ok(Some(TestOutputLine::TestResult {
                        name: name.to_string(),
                        status: TestStatus::Passed,
                        duration_ms,
                    }))
                } else {
                    Ok(None)
                }
            }
            (Some("test"), Some("failed")) => {
                if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                    let duration_ms = json.get("exec_time").and_then(|v| v.as_f64());

                    Ok(Some(TestOutputLine::TestResult {
                        name: name.to_string(),
                        status: TestStatus::Failed,
                        duration_ms,
                    }))
                } else {
                    Ok(None)
                }
            }
            (Some("test"), Some("ignored")) => {
                if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                    Ok(Some(TestOutputLine::TestResult {
                        name: name.to_string(),
                        status: TestStatus::Skipped,
                        duration_ms: None,
                    }))
                } else {
                    Ok(None)
                }
            }
            (Some("suite"), Some("failed")) | (Some("suite"), Some("ok")) => Ok(None),
            _ => Ok(None),
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

    pub fn read_pair(&mut self) -> Option<(String, String)> {
        let lines = self.take_lines(2);
        if lines.len() == 2 {
            Some((lines[0].clone(), lines[1].clone()))
        } else {
            None
        }
    }

    pub fn take_lines(&mut self, n: usize) -> Vec<String> {
        let mut lines = Vec::new();
        for _ in 0..n {
            if let Ok(Some(line)) = self.read_line() {
                lines.push(line);
            }
        }
        lines
    }

    pub fn read_panic_group(&mut self) -> Option<String> {
        let mut lines = Vec::new();
        let mut found_panic = false;

        for _ in 0..4 {
            if let Ok(Some(line)) = self.read_line() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if trimmed.starts_with("note: run with `RUST_BACKTRACE=1`")
                    || trimmed.starts_with("stack backtrace:")
                {
                    break;
                }

                if trimmed.starts_with("thread '") && trimmed.contains("' panicked at ") {
                    found_panic = true;
                }

                lines.push(line);

                if found_panic && lines.len() >= 2 {
                    break;
                }
            } else {
                break;
            }
        }

        if lines.len() >= 2 && found_panic {
            Some(lines.join("\n"))
        } else {
            None
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

impl<R: Read> Iterator for LineBuffer<R> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.read_line().unwrap_or(None)
    }
}
