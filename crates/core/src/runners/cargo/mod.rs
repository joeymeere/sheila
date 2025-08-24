pub mod types;
pub mod utils;

use mio::unix::pipe;
pub use types::*;
pub use utils::*;

use mio::{Events, Poll, Token, unix::SourceFd};
use serde::{Deserialize, Serialize};
use std::process::{ChildStderr, ChildStdout, Command, Stdio};
use std::rc::Rc;
use std::time::Duration;
use std::{
    io::{BufRead, BufReader},
    sync::mpsc::Sender,
};
use std::{os::fd::AsRawFd, path::PathBuf};
use strum_macros::{EnumDiscriminants, EnumString};

use crate::{
    Error, Result, RunnerConfig, TestRunner, TestSuite, runners::RunResult, suite::SuiteResult,
    test::TestResult,
};

const STDOUT_TOKEN: Token = Token(0);
const STDERR_TOKEN: Token = Token(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoRunnerConfig {
    /// Cargo profile to use (debug, release, etc.)
    pub profile: Option<String>,
    /// Additional cargo features to enable
    pub features: Vec<String>,
    /// Timeout for individual test executables
    pub executable_timeout: Option<Duration>,
    /// Whether to capture test output
    pub capture_output: bool,
    /// Additional cargo arguments
    pub cargo_args: Vec<String>,
    /// Additional test arguments passed to executables
    pub test_args: Vec<String>,
}

impl Default for CargoRunnerConfig {
    fn default() -> Self {
        Self {
            profile: None,
            features: vec!["sheila-proc-macros/__sheila_test".to_string()],
            executable_timeout: Some(Duration::from_secs(300)),
            capture_output: true,
            cargo_args: vec![],
            test_args: vec![],
        }
    }
}

#[derive(Debug, Clone, EnumDiscriminants)]
#[strum_discriminants(name(ProcessOutputType), derive(EnumString))]
pub enum ProcessOutput {
    /// A test has started running
    #[strum(serialize = "test_started")]
    TestStarted {
        name: String,
        suite: String,
    },
    /// A test completed successfully
    #[strum(serialize = "ok")]
    TestPassed {
        result: TestResult,
        duration_ms: f64,
    },
    /// A test failed with error details
    #[strum(serialize = "FAILED")]
    TestFailed {
        result: TestResult,
        duration_ms: f64,
        error: String,
    },
    /// A test was skipped
    #[strum(serialize = "test_skipped")]
    TestSkipped {
        result: TestResult,
    },
    /// An executable/suite has started
    #[strum(serialize = "running")]
    SuiteStarted {
        name: String,
        test_count: usize,
    },
    /// An executable/suite has completed (both stdout and stderr threads done)
    #[strum(serialize = "suite_completed")]
    SuiteCompleted {
        name: String,
    },
    Progress(TestResult),
    Error(TestResult),
    Done,
}

/// compiled test executable
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

    /// replace this with actual crate detection logic
    fn determine_target_crate(path: &PathBuf) -> String {
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
            "examples".to_string() // Default fallback
        }
    }
}

pub struct CargoTestRunner {
    poll: Poll,
    events: Events,
    state: TestRunState,
    output_tx: Option<Sender<ProcessOutput>>,
    config: RunnerConfig,
    cargo_config: CargoRunnerConfig,
}

impl CargoTestRunner {
    pub fn new(config: RunnerConfig) -> Self {
        Self {
            config,
            poll: Poll::new().unwrap(),
            events: Events::with_capacity(1024),
            state: TestRunState::new(),
            cargo_config: CargoRunnerConfig::default(),
            output_tx: None,
        }
    }

    pub fn new_with_output(config: RunnerConfig, output_tx: Sender<ProcessOutput>) -> Self {
        Self {
            config,
            poll: Poll::new().unwrap(),
            events: Events::with_capacity(1024),
            state: TestRunState::new(),
            cargo_config: CargoRunnerConfig::default(),
            output_tx: Some(output_tx),
        }
    }

    pub fn with_cargo_config(mut self, cargo_config: CargoRunnerConfig) -> Self {
        self.cargo_config = cargo_config;
        self
    }

    fn build_args(&self) -> Result<Vec<String>> {
        let mut cargo_args = vec![
            "test".to_string(),
            "--no-run".to_string(),
            "--message-format=json".to_string(),
        ];

        if let Some(ref profile) = self.cargo_config.profile {
            cargo_args.extend_from_slice(&["--profile".to_string(), profile.clone()]);
        }

        if !self.cargo_config.features.is_empty() {
            cargo_args.extend_from_slice(&[
                "--features".to_string(),
                self.cargo_config.features.join(","),
            ]);
        }

        cargo_args.extend_from_slice(&self.cargo_config.cargo_args);
        Ok(cargo_args)
    }

    fn test_args(&self) -> Vec<String> {
        let mut cargo_args = vec![
            "test".to_string(),
            "--no-run".to_string(),
            "--message-format=json".to_string(),
        ];

        if let Some(ref profile) = self.cargo_config.profile {
            cargo_args.extend_from_slice(&["--profile".to_string(), profile.clone()]);
        }

        if !self.cargo_config.features.is_empty() {
            cargo_args.extend_from_slice(&[
                "--features".to_string(),
                self.cargo_config.features.join(","),
            ]);
        }

        cargo_args.extend_from_slice(&self.cargo_config.cargo_args);
        cargo_args
    }

    pub fn build_executables(&self) -> Result<Vec<TestExecutable>> {
        let cargo_args = self.build_args()?;

        let mut child = Command::new("cargo")
            .args(&cargo_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::test_execution(format!("Failed to spawn cargo build: {}", e)))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::test_execution("Failed to capture cargo build stdout"))?;

        let mut executables = Vec::new();
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            let line = line.map_err(|e| {
                Error::test_execution(format!("Failed to read cargo output: {}", e))
            })?;

            if let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(executable) = self.extract_test_executable(&message)? {
                    executables.push(executable);
                }
            }
        }

        let exit_status = child
            .wait()
            .map_err(|e| Error::test_execution(format!("Failed to wait for cargo build: {}", e)))?;

        if !exit_status.success() {
            return Err(Error::test_execution(format!(
                "Cargo build failed with exit code: {:?}",
                exit_status.code()
            )));
        }

        Ok(executables)
    }

    fn extract_test_executable(
        &self,
        message: &serde_json::Value,
    ) -> Result<Option<TestExecutable>> {
        let reason = message
            .get("reason")
            .and_then(|r| r.as_str())
            .ok_or_else(|| Error::test_execution("Missing reason field in cargo output"))?;

        if reason != "compiler-artifact" {
            return Ok(None);
        }

        let package_id = message
            .get("package_id")
            .and_then(|p| p.as_str())
            .ok_or_else(|| Error::test_execution("Missing package_id field"))?;

        let package_name = package_id
            .split_whitespace()
            .next()
            .ok_or_else(|| Error::test_execution("Invalid package_id format"))?
            .to_string();

        let profile = message
            .get("profile")
            .ok_or_else(|| Error::test_execution("Missing profile field"))?;

        let is_test = profile
            .get("test")
            .and_then(|t| t.as_bool())
            .ok_or_else(|| Error::test_execution("Missing test field in profile"))?;

        if !is_test {
            return Ok(None);
        }

        if let Some(executable_path) = message.get("executable").and_then(|e| e.as_str()) {
            let target = message
                .get("target")
                .ok_or_else(|| Error::test_execution("Missing target field"))?;

            let name = target
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| Error::test_execution("Missing name field in target"))?
                .to_string();

            Ok(Some(TestExecutable::new(
                PathBuf::from(executable_path),
                name,
                package_name,
            )))
        } else {
            Ok(None)
        }
    }

    pub fn filter_executables(
        &self,
        executables: &[TestExecutable],
        target_filter: Option<&str>,
    ) -> Vec<TestExecutable> {
        if let Some(target) = target_filter {
            let target_crate = if target.contains('/') || target.ends_with(".rs") {
                TestExecutable::determine_target_crate(&PathBuf::from(target))
            } else {
                target.to_string()
            };

            executables
                .iter()
                .filter(|exe| {
                    exe.target_crate == target_crate
                        || exe.name.contains(&target_crate)
                        || exe.path.to_string_lossy().contains(&target_crate)
                        || (target_crate == "examples"
                            && (exe.name.contains("sheila_examples")
                                || exe.path.to_string_lossy().contains("sheila_examples")))
                })
                .cloned()
                .collect()
        } else {
            executables.to_vec()
        }
    }

    pub fn execute_tests(&mut self, executables: &[TestExecutable]) -> Result<RunResult> {
        let mut result = RunResult::new(self.config.clone());

        for executable in executables {
            let suite_result = match self.exec_test(executable.clone()) {
                Ok(result) => result,
                Err(e) => create_failed_suite_result(&executable.name, e),
            };

            let all_passed = suite_result.all_passed();
            result.add_suite_result(suite_result);

            if self.config.fail_fast && !all_passed {
                result.finish(Some(Error::test_execution(
                    "Failing fast due to test failure",
                )));
                break;
            }
        }

        if result.error.is_none() {
            result.finish(None);
        }

        Ok(result)
    }

    pub fn exec_test(&mut self, bin: TestExecutable) -> Result<SuiteResult> {
        let mut child = Command::new(&bin.path)
            .args(&self.test_args())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdout = Rc::new(child.stdout.take().unwrap());
        let mut stderr = Rc::new(child.stderr.take().unwrap());

        let mut stdout_buf = LineBuffer::new(*stdout.clone());
        let mut stderr_buf = LineBuffer::new(*stderr.clone());

        let stdout_pipe = pipe::Receiver::from(*stdout.clone());
        let stderr_pipe = pipe::Receiver::from(*stderr.clone());

        let _stdout_source = SourceFd(&stdout_pipe.as_raw_fd());
        let _stderr_source = SourceFd(&stderr_pipe.as_raw_fd());

        let mut test_results = Vec::new();

        loop {
            self.poll
                .poll(&mut self.events, Some(Duration::from_millis(100)))?;

            for event in self.events.iter() {
                match event.token() {
                    STDOUT_TOKEN => {
                        while let Some(line) = stdout_buf.read_line()? {
                            if let Ok((_, parsed)) = parse_test_output(&line) {
                                if let Some(output) = self.state.handle_line(parsed) {
                                    self.send_event(&output);

                                    match output {
                                        ProcessOutput::TestPassed { result, .. }
                                        | ProcessOutput::TestFailed { result, .. }
                                        | ProcessOutput::TestSkipped { result } => {
                                            test_results.push(result);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    STDERR_TOKEN => {
                        while let Some(line) = stderr_buf.read_line()? {
                            if let Ok((_, parsed)) = parse_error_output(&line) {
                                self.state.handle_line(parsed);
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            }

            match child.try_wait()? {
                Some(_status) => {
                    self.flush_buffers(&mut stdout_buf, &mut stderr_buf, &mut test_results)?;

                    return Ok(create_suite_result(&bin.name, &test_results));
                }
                None => continue,
            }
        }
    }

    fn send_event(&self, output: &ProcessOutput) {
        if let Some(ref tx) = self.output_tx {
            let _ = tx.send(output.clone());
        }
    }

    fn flush_buffers(
        &mut self,
        stdout_buf: &mut LineBuffer<ChildStdout>,
        stderr_buf: &mut LineBuffer<ChildStderr>,
        test_results: &mut Vec<TestResult>,
    ) -> Result<()> {
        if let Some(line) = stdout_buf.flush_remaining() {
            if let Ok((_, parsed)) = parse_test_output(&line) {
                if let Some(output) = self.state.handle_line(parsed) {
                    self.send_event(&output);

                    match output {
                        ProcessOutput::TestPassed { result, .. }
                        | ProcessOutput::TestFailed { result, .. }
                        | ProcessOutput::TestSkipped { result } => {
                            test_results.push(result);
                        }
                        _ => {}
                    }
                }
            }
        }

        if let Some(line) = stderr_buf.flush_remaining() {
            if let Ok((_, parsed)) = parse_error_output(&line) {
                self.state.handle_line(parsed);
            }
        }

        self.state.finalize_pending_errors(test_results);

        Ok(())
    }
}

impl Default for CargoTestRunner {
    fn default() -> Self {
        Self::new(RunnerConfig::default())
    }
}

impl TestRunner for CargoTestRunner {
    fn run(&self, suites: Vec<TestSuite>) -> Result<RunResult> {
        let mut result = RunResult::new(self.config.clone());

        let suites_to_run = self.filter_suites(suites);

        if suites_to_run.is_empty() {
            result.finish(None);
            return Ok(result);
        }

        for mut suite in suites_to_run {
            match suite.execute() {
                Ok(suite_result) => {
                    let should_fail_fast = self.config.fail_fast && !suite_result.all_passed();
                    result.add_suite_result(suite_result);

                    if should_fail_fast {
                        result.finish(Some(Error::test_execution(
                            "Failing fast due to test failure",
                        )));
                        return Ok(result);
                    }
                }
                Err(e) => {
                    result.finish(Some(e));
                    return Ok(result);
                }
            }
        }

        result.finish(None);
        Ok(result)
    }

    fn run_suite(&self, mut suite: TestSuite) -> Result<SuiteResult> {
        suite.execute()
    }

    fn config(&self) -> &RunnerConfig {
        &self.config
    }

    fn set_config(&mut self, config: RunnerConfig) {
        self.config = config;
    }
}
