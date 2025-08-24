#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::time::Instant;
use uuid::Uuid;

use nom::{
    IResult,
    bytes::complete::{tag, tag_no_case, take_until, take_while1},
    character::complete::{digit1, space1},
    combinator::{map, opt},
    sequence::{delimited, preceded, tuple},
};

use nom::branch::alt;

use crate::{
    Error, RunnerConfig, TestMetadata, TestStatus,
    runners::{ProcessOutput, RunResult, TestOutputLine, TestRunState},
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
        self.test_state
            .handle_line(TestOutputLine::PanicMessage { message: err });
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

// #[derive(Debug, Clone, Default)]
// pub(crate) struct OutputParser {
//     pub output_lines: Vec<String>,
// }

// impl OutputParser {
//     pub fn new() -> Self {
//         Self {
//             output_lines: Vec::new(),
//         }
//     }

//     pub fn push(&mut self, line: String) {
//         self.output_lines.push(line);
//     }

//     pub fn get_lines(&self) -> &[String] {
//         &self.output_lines
//     }

//     pub fn clear(&mut self) {
//         self.output_lines.clear();
//     }

//     pub fn parse_test_line(
//         line: &str,
//         suite_name: &str,
//         duration_ms: f64,
//     ) -> Option<ProcessOutput> {
//         // completion + timing: "test foo ... ok" or "test foo ... FAILED"
//         if line.starts_with("test ") && (line.contains(" ... ok") || line.contains(" ... FAILED")) {
//             let parts: Vec<&str> = line.split_whitespace().collect();
//             if parts.len() >= 4 {
//                 let test_name = parts[1].to_string();
//                 let status = parts[3];

//                 match status {
//                     "ok" => {
//                         let test_result = Self::create_test_result(&test_name, status);
//                         return Some(ProcessOutput::TestPassed {
//                             result: test_result,
//                             duration_ms,
//                         });
//                     }
//                     "FAILED" => {
//                         let test_result = Self::create_test_result(&test_name, status);
//                         // Error details will be parsed from subsequent lines
//                         return Some(ProcessOutput::TestFailed {
//                             result: test_result,
//                             duration_ms,
//                             error: String::new(),
//                         });
//                     }
//                     _ => {
//                         let test_result = Self::create_test_result(&test_name, status);
//                         return Some(ProcessOutput::TestSkipped {
//                             result: test_result,
//                         });
//                     }
//                 }
//             }
//         }

//         // Test starting: "running 1 test" or similar patterns
//         if line.contains("running") && line.contains("test") {
//             if let Some(count) = line
//                 .split_whitespace()
//                 .find(|s| s.parse::<usize>().is_ok())
//                 .and_then(|s| s.parse::<usize>().ok())
//             {
//                 return Some(ProcessOutput::SuiteStarted {
//                     name: suite_name.to_string(),
//                     test_count: count,
//                 });
//             }
//         }

//         None
//     }

//     pub fn parse_error_context(lines: &[String], test_name: &str) -> String {
//         let mut file_location = None;
//         let mut error_message = None;

//         for (i, line) in lines.iter().enumerate() {
//             let trimmed = line.trim();

//             // "thread 'test_name' panicked at examples/src/filesystem.rs:243:9:"
//             if trimmed.contains("panicked at") && trimmed.contains(&test_name) {
//                 if let Some(panic_start) = trimmed.find("panicked at ") {
//                     let location_part = &trimmed[panic_start + 12..];
//                     if let Some(colon_pos) = location_part.rfind(':') {
//                         file_location = Some(location_part[..colon_pos].to_string());
//                     }
//                 }

//                 if i + 1 < lines.len() {
//                     let next_line = lines[i + 1].trim();
//                     if !next_line.is_empty()
//                         && !next_line.starts_with("note:")
//                         && !next_line.starts_with("stack backtrace:")
//                     {
//                         error_message = Some(next_line.to_string());
//                     }
//                 }
//                 break;
//             }
//         }

//         match (file_location, error_message) {
//             (Some(location), Some(message)) => {
//                 // get line + col (ex., "examples/src/filesystem.rs:243:9")
//                 let parts: Vec<&str> = location.split(':').collect();
//                 if parts.len() >= 3 {
//                     let file_path = parts[0];
//                     let line_num = parts[1];
//                     let col_num = parts[2];

//                     format!(
//                         "--> {}:{}:{}\n    |\n{} | <source code placeholder>\n    | {}^ {}\n    |",
//                         file_path,
//                         line_num,
//                         col_num,
//                         line_num,
//                         " ".repeat(col_num.parse::<usize>().unwrap_or(0).saturating_sub(1)),
//                         message
//                     )
//                 } else {
//                     format!("--> {}\n    {}", location, message)
//                 }
//             }
//             (Some(location), None) => {
//                 format!("--> {}\n    Test failed at this location", location)
//             }
//             (None, Some(message)) => {
//                 format!("Test panicked: {}", message)
//             }
//             (None, None) => {
//                 format!(
//                     "Test '{}' failed - check test output for details",
//                     test_name
//                 )
//             }
//         }
//     }

//     pub fn create_test_result(name: &str, status: &str) -> TestResult {
//         let test_id = Uuid::new_v4();
//         let name = format_mod_name(name);

//         let metadata = TestMetadata::new(name.clone());
//         let mut test_result = TestResult::new(test_id, name, metadata);

//         match status {
//             "ok" => test_result.finish(TestStatus::Passed, None),
//             "FAILED" => test_result.finish(
//                 TestStatus::Failed,
//                 Some(Error::test_execution("Test failed")),
//             ),
//             _ => test_result.finish(TestStatus::Skipped, None),
//         }

//         test_result
//     }

//     pub fn create_suite_result(
//         &self,
//         suite_name: &str,
//         test_results: &[TestResult],
//     ) -> SuiteResult {
//         let suite_id = Uuid::new_v4();
//         let name = format_mod_name(suite_name);
//         let metadata = TestMetadata::new(name.clone());
//         let mut suite_result = SuiteResult::new(suite_id, name, metadata);

//         for test_result in test_results {
//             suite_result.add_test_result(test_result.clone());
//         }

//         suite_result
//     }
// }

#[derive(Debug, Clone, Default)]
pub(crate) struct OutputParser {
    pub output_lines: Vec<String>,
}

impl OutputParser {
    pub fn new() -> Self {
        Self {
            output_lines: Vec::new(),
        }
    }

    pub fn push(&mut self, line: String) {
        self.output_lines.push(line);
    }

    pub fn get_lines(&self) -> &[String] {
        &self.output_lines
    }

    pub fn clear(&mut self) {
        self.output_lines.clear();
    }

    pub fn parse_test_line(
        line: &str,
        suite_name: &str,
        duration_ms: f64,
    ) -> Option<ProcessOutput> {
        // completion + timing: "test foo ... ok" or "test foo ... FAILED"
        if line.starts_with("test ") && (line.contains(" ... ok") || line.contains(" ... FAILED")) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let test_name = parts[1].to_string();
                let status = parts[3];

                match status {
                    "ok" => {
                        let test_result = Self::create_test_result(&test_name, status);
                        return Some(ProcessOutput::TestPassed {
                            result: test_result,
                            duration_ms,
                        });
                    }
                    "FAILED" => {
                        let test_result = Self::create_test_result(&test_name, status);
                        // Error details will be parsed from subsequent lines
                        return Some(ProcessOutput::TestFailed {
                            result: test_result,
                            duration_ms,
                            error: String::new(),
                        });
                    }
                    _ => {
                        let test_result = Self::create_test_result(&test_name, status);
                        return Some(ProcessOutput::TestSkipped {
                            result: test_result,
                        });
                    }
                }
            }
        }

        // Test starting: "running 1 test" or similar patterns
        if line.contains("running") && line.contains("test") {
            if let Some(count) = line
                .split_whitespace()
                .find(|s| s.parse::<usize>().is_ok())
                .and_then(|s| s.parse::<usize>().ok())
            {
                return Some(ProcessOutput::SuiteStarted {
                    name: suite_name.to_string(),
                    test_count: count,
                });
            }
        }

        None
    }

    pub fn parse_error_context(lines: &[String], test_name: &str) -> String {
        let mut file_location = None;
        let mut error_message = None;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // "thread 'test_name' panicked at examples/src/filesystem.rs:243:9:"
            if trimmed.contains("panicked at") && trimmed.contains(&test_name) {
                if let Some(panic_start) = trimmed.find("panicked at ") {
                    let location_part = &trimmed[panic_start + 12..];
                    if let Some(colon_pos) = location_part.rfind(':') {
                        file_location = Some(location_part[..colon_pos].to_string());
                    }
                }

                if i + 1 < lines.len() {
                    let next_line = lines[i + 1].trim();
                    if !next_line.is_empty()
                        && !next_line.starts_with("note:")
                        && !next_line.starts_with("stack backtrace:")
                    {
                        error_message = Some(next_line.to_string());
                    }
                }
                break;
            }
        }

        match (file_location, error_message) {
            (Some(location), Some(message)) => {
                // get line + col (ex., "examples/src/filesystem.rs:243:9")
                let parts: Vec<&str> = location.split(':').collect();
                if parts.len() >= 3 {
                    let file_path = parts[0];
                    let line_num = parts[1];
                    let col_num = parts[2];

                    format!(
                        "--> {}:{}:{}\n    |\n{} | <source code placeholder>\n    | {}^ {}\n    |",
                        file_path,
                        line_num,
                        col_num,
                        line_num,
                        " ".repeat(col_num.parse::<usize>().unwrap_or(0).saturating_sub(1)),
                        message
                    )
                } else {
                    format!("--> {}\n    {}", location, message)
                }
            }
            (Some(location), None) => {
                format!("--> {}\n    Test failed at this location", location)
            }
            (None, Some(message)) => {
                format!("Test panicked: {}", message)
            }
            (None, None) => {
                format!(
                    "Test '{}' failed - check test output for details",
                    test_name
                )
            }
        }
    }

    pub fn create_test_result(name: &str, status: &str) -> TestResult {
        let test_id = Uuid::new_v4();
        let name = format_mod_name(name);

        let metadata = TestMetadata::new(name.clone());
        let mut test_result = TestResult::new(test_id, name, metadata);

        match status {
            "ok" => test_result.finish(TestStatus::Passed, None),
            "FAILED" => test_result.finish(
                TestStatus::Failed,
                Some(Error::test_execution("Test failed")),
            ),
            _ => test_result.finish(TestStatus::Skipped, None),
        }

        test_result
    }

    pub fn create_suite_result(
        &self,
        suite_name: &str,
        test_results: &[TestResult],
    ) -> SuiteResult {
        let suite_id = Uuid::new_v4();
        let name = format_mod_name(suite_name);
        let metadata = TestMetadata::new(name.clone());
        let mut suite_result = SuiteResult::new(suite_id, name, metadata);

        for test_result in test_results {
            suite_result.add_test_result(test_result.clone());
        }

        suite_result
    }
}

#[cfg(unix)]
pub(crate) fn set_nonblocking<T: AsRawFd>(fd: &T) -> std::io::Result<()> {
    use libc::{F_GETFL, F_SETFL, O_NONBLOCK, fcntl};

    let raw_fd = fd.as_raw_fd();

    unsafe {
        let flags = fcntl(raw_fd, F_GETFL, 0);
        if flags == -1 {
            return Err(std::io::Error::last_os_error());
        }

        let result = fcntl(raw_fd, F_SETFL, flags | O_NONBLOCK);
        if result == -1 {
            return Err(std::io::Error::last_os_error());
        }
    }

    Ok(())
}

pub fn format_mod_name(name: &str) -> String {
    if name.contains("__sheila_") {
        let re = regex::Regex::new(r"::(__sheila_[^:]+_tests)::").unwrap();
        let name = re.replace(name, "::");
        name.to_string()
    } else {
        name.to_string()
    }
}

pub fn parse_test_output(input: &str) -> IResult<&str, TestOutputLine> {
    alt((parse_test_result, parse_test_start, parse_suite_start))(input)
}

pub fn parse_error_output(input: &str) -> IResult<&str, TestOutputLine> {
    alt((parse_panic_location, parse_panic_message))(input)
}

pub fn parse_panic_message(input: &str) -> IResult<&str, TestOutputLine> {
    let (input, _) = tag("thread '")(input)?;
    let (input, test_name) = take_until("'")(input)?;
    let (input, _) = tag("' panicked at ")(input)?;
    let (input, message) = take_until("\n")(input)?;
    Ok((
        input,
        TestOutputLine::PanicMessage {
            message: message.to_string(),
        },
    ))
}

pub fn parse_test_start(input: &str) -> IResult<&str, TestOutputLine> {
    let (input, _) = tag("test ")(input)?;
    let (input, name) = take_while1(|c: char| !c.is_whitespace())(input)?;

    Ok((
        input,
        TestOutputLine::TestStart {
            name: name.to_string(),
        },
    ))
}

pub fn parse_suite_start(input: &str) -> IResult<&str, TestOutputLine> {
    let (input, _) = tag("running ")(input)?;
    let (input, count) = map(digit1, |s: &str| s.parse::<usize>().unwrap_or(0))(input)?;
    let (input, _) = tag(" test")(input)?;

    Ok((input, TestOutputLine::SuiteStart { count }))
}

// Parse "test module::test_name ... ok" or "test module::test_name ... FAILED"
pub fn parse_test_result(input: &str) -> IResult<&str, TestOutputLine> {
    let (input, _) = tag_no_case("test ")(input)?;
    let (input, name) = take_until(" ")(input)?;
    let (input, _) = tag(" ... ")(input)?;
    let (input, status) = alt((
        map(tag("ok"), |_| TestStatus::Passed),
        map(tag("FAILED"), |_| TestStatus::Failed),
        map(tag("ignored"), |_| TestStatus::Skipped),
    ))(input)?;

    Ok((
        input,
        TestOutputLine::TestResult {
            name: name.to_string(),
            status,
            duration_ms: None, // Parse timing if present
        },
    ))
}

// Parse panic location: "thread 'test_name' panicked at src/lib.rs:42:15:"
pub fn parse_panic_location(input: &str) -> IResult<&str, TestOutputLine> {
    let (input, _) = tag("thread '")(input)?;
    let (input, test_name) = take_until("'")(input)?;
    let (input, _) = tag("' panicked at ")(input)?;
    let (input, file) = take_until(":")(input)?;
    let (input, _) = tag(":")(input)?;
    let (input, line) = map(digit1, |s: &str| s.parse::<u32>().unwrap())(input)?;
    let (input, _) = tag(":")(input)?;
    let (input, column) = map(digit1, |s: &str| s.parse::<u32>().unwrap())(input)?;

    Ok((
        input,
        TestOutputLine::PanicLocation {
            test: test_name.to_string(),
            file: file.to_string(),
            line,
            column,
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

pub fn create_failed_run_result(suite_name: &str, error: Error) -> RunResult {
    let name = format_mod_name(suite_name);
    let mut run_result = RunResult::new(RunnerConfig::default());

    let suite_result = create_failed_suite_result(&name, error);
    run_result.add_suite_result(suite_result);

    run_result
}

pub fn create_failed_suite_result(suite_name: &str, error: Error) -> SuiteResult {
    let suite_id = Uuid::new_v4();
    let name = format_mod_name(suite_name);
    let metadata = TestMetadata::new(name.clone());
    let mut suite_result = SuiteResult::new(suite_id, name.clone(), metadata);

    let mut test_result = TestResult::new(
        Uuid::new_v4(),
        format!("{}_system_error", suite_name),
        TestMetadata::new(format!("{} (system error)", name)),
    );
    test_result.finish(TestStatus::Failed, Some(error));
    suite_result.add_test_result(test_result);

    suite_result
}
