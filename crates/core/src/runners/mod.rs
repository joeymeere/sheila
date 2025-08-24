#[cfg(feature = "cargo-compat")]
pub mod cargo;
#[cfg(feature = "cargo-compat")]
pub use cargo::*;

pub mod thin;
pub use thin::*;

use crate::suite::SuiteResult;
use crate::{Error, Result, TestSuite};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub trait TestRunner: Send + Sync {
    fn run(&self, suites: Vec<TestSuite>) -> Result<RunResult>;

    fn run_suite(&self, suite: TestSuite) -> Result<SuiteResult>;

    fn config(&self) -> &RunnerConfig;

    fn set_config(&mut self, config: RunnerConfig);

    fn filter_suites(&self, suites: Vec<TestSuite>) -> Vec<TestSuite> {
        let config = self.config();

        suites
            .into_iter()
            .filter(|suite| {
                if suite.attributes.ignore {
                    return false;
                }

                if !config.include_patterns.is_empty() {
                    let matches = config.include_patterns.iter().any(|pattern| {
                        suite.name.contains(pattern) || suite.meta.name.contains(pattern)
                    });
                    if !matches {
                        return false;
                    }
                }

                if !config.exclude_patterns.is_empty() {
                    let matches = config.exclude_patterns.iter().any(|pattern| {
                        suite.name.contains(pattern) || suite.meta.name.contains(pattern)
                    });
                    if matches {
                        return false;
                    }
                }

                if !config.include_tags.is_empty() {
                    let has_tag = config
                        .include_tags
                        .iter()
                        .any(|tag| suite.attributes.tags.contains(tag));
                    if !has_tag {
                        return false;
                    }
                }

                if !config.exclude_tags.is_empty() {
                    let has_tag = config
                        .exclude_tags
                        .iter()
                        .any(|tag| suite.attributes.tags.contains(tag));
                    if has_tag {
                        return false;
                    }
                }

                if !config.include_categories.is_empty() {
                    if let Some(ref category) = suite.attributes.category {
                        if !config.include_categories.contains(category) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                if !config.exclude_categories.is_empty() {
                    if let Some(ref category) = suite.attributes.category {
                        if config.exclude_categories.contains(category) {
                            return false;
                        }
                    }
                }

                true
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    /// Maximum number of concurrent test suites
    pub max_concurrent_suites: Option<usize>,
    /// Default timeout for tests
    pub default_test_timeout: Option<Duration>,
    /// Default timeout for test suites
    pub default_suite_timeout: Option<Duration>,
    /// Whether to stop on first failure
    pub fail_fast: bool,
    /// Whether to run tests in parallel
    pub parallel: bool,
    /// Test patterns to include
    pub include_patterns: Vec<String>,
    /// Test patterns to exclude
    pub exclude_patterns: Vec<String>,
    /// Tags to include
    pub include_tags: Vec<String>,
    /// Tags to exclude
    pub exclude_tags: Vec<String>,
    /// Categories to include
    pub include_categories: Vec<String>,
    /// Categories to exclude
    pub exclude_categories: Vec<String>,
    /// Output directory for reports
    pub output_dir: Option<PathBuf>,
    /// Whether to capture stdout/stderr
    pub capture_output: bool,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Custom configuration
    pub custom: HashMap<String, serde_json::Value>,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_suites: Some(4),
            default_test_timeout: Some(Duration::from_secs(30)),
            default_suite_timeout: Some(Duration::from_secs(300)),
            fail_fast: false,
            parallel: true,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            include_tags: Vec::new(),
            exclude_tags: Vec::new(),
            include_categories: Vec::new(),
            exclude_categories: Vec::new(),
            output_dir: None,
            capture_output: true,
            env: HashMap::new(),
            custom: HashMap::new(),
        }
    }
}

impl RunnerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum concurrent suites
    pub fn max_concurrent_suites(mut self, max: usize) -> Self {
        self.max_concurrent_suites = Some(max);
        self
    }

    /// Set default test timeout
    pub fn default_test_timeout(mut self, timeout: Duration) -> Self {
        self.default_test_timeout = Some(timeout);
        self
    }

    /// Set default suite timeout
    pub fn default_suite_timeout(mut self, timeout: Duration) -> Self {
        self.default_suite_timeout = Some(timeout);
        self
    }

    /// Enable/disable fail fast
    pub fn fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    /// Enable/disable parallel execution
    pub fn parallel(mut self, parallel: bool) -> Self {
        self.parallel = parallel;
        self
    }

    /// Add include pattern
    pub fn include_pattern<S: Into<String>>(mut self, pattern: S) -> Self {
        self.include_patterns.push(pattern.into());
        self
    }

    /// Add exclude pattern
    pub fn exclude_pattern<S: Into<String>>(mut self, pattern: S) -> Self {
        self.exclude_patterns.push(pattern.into());
        self
    }

    /// Add include tag
    pub fn include_tag<S: Into<String>>(mut self, tag: S) -> Self {
        self.include_tags.push(tag.into());
        self
    }

    /// Add exclude tag
    pub fn exclude_tag<S: Into<String>>(mut self, tag: S) -> Self {
        self.exclude_tags.push(tag.into());
        self
    }

    /// Set output directory
    pub fn output_dir<P: Into<PathBuf>>(mut self, dir: P) -> Self {
        self.output_dir = Some(dir.into());
        self
    }

    /// Add environment variable
    pub fn env<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.env.insert(key.into(), value.into());
        self
    }
}

/// Overall test run result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    /// Run ID
    pub id: Uuid,
    /// Start time
    pub start_time: DateTime<Utc>,
    /// End time
    pub end_time: Option<DateTime<Utc>>,
    /// Duration
    pub duration: Option<Duration>,
    /// Suite results
    pub suite_results: Vec<SuiteResult>,
    /// Configuration used
    pub config: RunnerConfig,
    /// Total suites
    pub total_suites: usize,
    /// Passed suites
    pub passed_suites: usize,
    /// Failed suites
    pub failed_suites: usize,
    /// Skipped suites
    pub skipped_suites: usize,
    /// Total tests across all suites
    pub total_tests: usize,
    /// Passed tests across all suites
    pub passed_tests: usize,
    /// Failed tests across all suites
    pub failed_tests: usize,
    /// Skipped tests across all suites
    pub skipped_tests: usize,
    /// Overall error (if any)
    pub error: Option<Error>,
}

impl RunResult {
    /// Create a new run result
    pub fn new(config: RunnerConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            start_time: Utc::now(),
            end_time: None,
            duration: None,
            suite_results: Vec::new(),
            config,
            total_suites: 0,
            passed_suites: 0,
            failed_suites: 0,
            skipped_suites: 0,
            total_tests: 0,
            passed_tests: 0,
            failed_tests: 0,
            skipped_tests: 0,
            error: None,
        }
    }

    /// Add a suite result
    pub fn add_suite_result(&mut self, result: SuiteResult) {
        self.total_suites += 1;
        self.total_tests += result.total_tests;
        self.passed_tests += result.passed_tests;
        self.failed_tests += result.failed_tests;
        self.skipped_tests += result.skipped_tests;

        if result.all_passed() {
            self.passed_suites += 1;
        } else if result.failed_tests > 0 || result.error.is_some() {
            self.failed_suites += 1;
        } else {
            self.skipped_suites += 1;
        }

        self.suite_results.push(result);
    }

    /// Finish the run
    pub fn finish(&mut self, error: Option<Error>) {
        self.end_time = Some(Utc::now());
        self.error = error;

        if let Some(end_time) = self.end_time {
            self.duration = Some(Duration::from_millis(
                (end_time - self.start_time).num_milliseconds() as u64,
            ));
        }
    }

    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.failed_tests == 0 && self.failed_suites == 0 && self.error.is_none()
    }

    /// Get overall success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_tests == 0 {
            return 1.0;
        }
        self.passed_tests as f64 / self.total_tests as f64
    }

    /// Get suite success rate
    pub fn suite_success_rate(&self) -> f64 {
        if self.total_suites == 0 {
            return 1.0;
        }
        self.passed_suites as f64 / self.total_suites as f64
    }
}

pub type SharedTracker = Arc<RwLock<TestTracker>>;

#[derive(Debug, Clone)]
pub struct TestTracker {
    pub current_test: Option<String>,
    pub previous_test: Option<String>,
    pub timer: Instant,
    pub err_meta: Option<String>,
}

impl TestTracker {
    pub fn new() -> Self {
        Self {
            current_test: None,
            previous_test: None,
            timer: Instant::now(),
            err_meta: None,
        }
    }

    pub fn start_test(&mut self, test_name: String) {
        self.previous_test = self.current_test.clone();
        self.current_test = Some(test_name);
        self.timer = Instant::now();
    }

    pub fn end_test(&mut self) {
        self.previous_test = self.current_test.clone();
        self.current_test = None;
    }

    pub fn end_test_with_error(&mut self, err_meta: String) {
        self.previous_test = self.current_test.clone();
        self.current_test = None;
        self.err_meta = Some(err_meta);
    }

    pub fn set_error(&mut self, err_meta: String) {
        self.err_meta = Some(err_meta);
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
