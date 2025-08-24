use crate::{
    Error, Result, RunnerConfig, TestRunner, TestSuite, runners::RunResult, suite::SuiteResult,
};

pub struct DefaultTestRunner {
    config: RunnerConfig,
}

impl DefaultTestRunner {
    pub fn new(config: RunnerConfig) -> Self {
        Self { config }
    }
}

impl Default for DefaultTestRunner {
    fn default() -> Self {
        Self::new(RunnerConfig::default())
    }
}

impl TestRunner for DefaultTestRunner {
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
