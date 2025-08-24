pub mod utils;
pub use utils::*;

use mio::Interest;
use mio::unix::pipe;
use mio::{Events, Poll};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::Duration;
use uuid::Uuid;

use crate::{
    Error, Result, RunnerConfig, TestRunner, TestSuite, runners::RunResult, suite::SuiteResult,
    test::TestResult,
};
use crate::{
    JsonLineParser, LineBuffer, ProcessOutput, STDERR_TOKEN, STDOUT_TOKEN, StandardLineParser,
    TestExecutable, TestMetadata, TestRunState,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoRunnerConfig {
    pub profile: Option<String>,
    pub features: Vec<String>,
    pub executable_timeout: Option<Duration>,
    pub capture_output: bool,
    pub cargo_args: Vec<String>,
    pub test_args: Vec<String>,
}

impl Default for CargoRunnerConfig {
    fn default() -> Self {
        Self {
            profile: None,
            features: vec![
                "sheila-proc-macros/__sheila_test".to_string(),
                "sheila/full".to_string(),
            ],
            executable_timeout: Some(Duration::from_secs(300)),
            capture_output: true,
            cargo_args: vec![],
            test_args: vec![],
        }
    }
}

pub struct CargoTestRunner {
    pub poll: Poll,
    pub events: Events,
    pub state: TestRunState,
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

    fn args(&self) -> Vec<String> {
        let mut test_args = vec![
            "--format=json".to_string(),
            "--report-time".to_string(),
            "-Z".to_string(),
            "unstable-options".to_string(),
        ];
        test_args.extend_from_slice(&self.cargo_config.test_args);
        test_args
    }

    pub fn execute_tests(&mut self, executables: &[TestExecutable]) -> Result<RunResult> {
        let mut result = RunResult::new(self.config.clone());

        for executable in executables {
            let suite_result = match self.exec_test(executable.clone()) {
                Ok(result) => result,
                Err(e) => StandardLineParser::create_suite_result(
                    &executable.name,
                    &[TestResult::new(
                        Uuid::new_v4(),
                        format!("{}_system_error", executable.name),
                        TestMetadata::new(format!("{} (system error)", e)),
                    )],
                ),
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
        let test_args = self.args();

        let mut child = Command::new(&bin.path)
            .args(&test_args)
            .env("RUST_TEST_NOCAPTURE", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let mut stdout_pipe = pipe::Receiver::from(stdout);
        let mut stderr_pipe = pipe::Receiver::from(stderr);

        stdout_pipe.set_nonblocking(true)?;
        stderr_pipe.set_nonblocking(true)?;

        self.poll
            .registry()
            .register(&mut stdout_pipe, STDOUT_TOKEN, Interest::READABLE)?;

        self.poll
            .registry()
            .register(&mut stderr_pipe, STDERR_TOKEN, Interest::READABLE)?;

        let mut stdout_buf = LineBuffer::new(stdout_pipe);
        let mut stderr_buf = LineBuffer::new(stderr_pipe);

        let mut test_results = Vec::new();

        loop {
            self.poll
                .poll(&mut self.events, Some(Duration::from_millis(100)))?;

            for event in self.events.iter() {
                match event.token() {
                    STDOUT_TOKEN => {
                        while let Some(line) = stdout_buf.read_line()? {
                            match JsonLineParser::parse_test_output(&line) {
                                Ok(Some(parsed)) => {
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
                                Ok(None) => {}
                                Err(_) => {
                                    if let Ok((_, parsed)) =
                                        StandardLineParser::parse_test_output(&line)
                                    {
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
                        }
                    }
                    STDERR_TOKEN => {
                        while let Some(panic_group) = stderr_buf.read_panic_group() {
                            match StandardLineParser::parse_error_output(&panic_group) {
                                Ok(parsed) => {
                                    let output = match self.state.handle_line(parsed) {
                                        Some(output) => output,
                                        None => continue,
                                    };
                                    self.send_event(&output);
                                }
                                Err(_) => {
                                    continue;
                                }
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            }

            match child.try_wait()? {
                Some(_status) => {
                    self.flush_buffers(&mut stdout_buf, &mut stderr_buf, &mut test_results)?;

                    return Ok(StandardLineParser::create_suite_result(
                        &bin.name,
                        &test_results,
                    ));
                }
                None => continue,
            }
        }
    }

    fn send_event(&self, output: &ProcessOutput) {
        if let Some(ref tx) = self.output_tx {
            if let Err(e) = tx.send(output.clone()) {
                dbg!(&e);
            }
        }
    }

    fn flush_buffers(
        &mut self,
        stdout_buf: &mut LineBuffer<pipe::Receiver>,
        stderr_buf: &mut LineBuffer<pipe::Receiver>,
        test_results: &mut Vec<TestResult>,
    ) -> Result<()> {
        if let Some(line) = stdout_buf.flush_remaining() {
            match JsonLineParser::parse_test_output(&line) {
                Ok(Some(parsed)) => {
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
                Ok(None) | Err(_) => {
                    if let Ok((_, parsed)) = StandardLineParser::parse_test_output(&line) {
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
        }

        if let Some(line) = stderr_buf.flush_remaining() {
            match StandardLineParser::parse_error_output(&line) {
                Ok(parsed) => {
                    if let Some(output) = self.state.handle_line(parsed) {
                        self.send_event(&output);
                    }
                }
                Err(_) => {}
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
