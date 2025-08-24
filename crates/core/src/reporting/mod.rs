pub mod formats;
pub mod logging;
pub mod types;

pub use formats::*;
pub use logging::*;
pub use types::*;

use crate::runners::RunResult;

use crate::{Error, Result};
use chrono::Utc;
use std::io::Write;
use std::path::Path;

/// Define a custom reporter that can be used to structure the results of a test run.
///
/// The interface specifies two methods:
///
/// - `generate`: used to generate a report from the run results.
/// - `format`: used to return the reporting format that this reporter generates.
///
/// See the [`ReportFormat` enum](./types.rs#ReportFormat) for more details.
pub trait Reporter: Send + Sync {
    /// Generate a report from run results
    fn generate(&self, run_result: &RunResult) -> Result<TestReport>;

    /// Return the reporting format that this reporter generates
    ///
    /// See the [`ReportFormat` enum](./types.rs#ReportFormat) for more details.
    fn format(&self) -> ReportFormat;
}

/// Reporter extension trait for the purpose of writing reports to some
/// output I/O stream. Generally, this will be a file or the stdout.
pub trait ReporterExt {
    /// Write a report output to a generic writer
    fn write_report<W: Write>(&self, report: &TestReport, writer: &mut W) -> Result<()> {
        writer
            .write_all(report.content.as_bytes())
            .map_err(Error::from)
    }

    /// Write a report output to a file
    fn write_file(&self, report: &TestReport, path: &Path) -> Result<()> {
        let mut file = std::fs::File::create(path).map_err(Error::from)?;
        self.write_report(report, &mut file)
    }
}

impl<T: Reporter> ReporterExt for T {}

pub struct TextReporter {
    metadata: ReportMetadata,
    show_details: bool,
    show_timing: bool,
}

impl TextReporter {
    pub fn new() -> Self {
        Self {
            metadata: ReportMetadata::default(),
            show_details: true,
            show_timing: true,
        }
    }

    pub fn with_metadata(mut self, metadata: ReportMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn show_timing(mut self, show: bool) -> Self {
        self.show_timing = show;
        self
    }

    fn format_duration(duration: &std::time::Duration) -> String {
        let millis = duration.as_millis();
        if millis < 1000 {
            format!("{}ms", millis)
        } else {
            format!("{:.2}s", duration.as_secs_f64())
        }
    }
}

impl Default for TextReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for TextReporter {
    fn generate(&self, run_result: &RunResult) -> Result<TestReport> {
        let mut content = String::new();

        content.push_str(&format!("# {}\n\n", self.metadata.title));

        if let Some(ref description) = self.metadata.description {
            content.push_str(&format!("{}\n\n", description));
        }

        content.push_str("## Summary\n\n");
        content.push_str(&format!("Total Suites: {}\n", run_result.total_suites));
        content.push_str(&format!("Passed Suites: {}\n", run_result.passed_suites));
        content.push_str(&format!("Failed Suites: {}\n", run_result.failed_suites));
        content.push_str(&format!("Skipped Suites: {}\n", run_result.skipped_suites));
        content.push_str(&format!("Total Tests: {}\n", run_result.total_tests));
        content.push_str(&format!("Passed Tests: {}\n", run_result.passed_tests));
        content.push_str(&format!("Failed Tests: {}\n", run_result.failed_tests));
        content.push_str(&format!("Skipped Tests: {}\n", run_result.skipped_tests));
        content.push_str(&format!(
            "Success Rate: {:.1}%\n",
            run_result.success_rate() * 100.0
        ));

        if self.show_timing {
            if let Some(ref duration) = run_result.duration {
                content.push_str(&format!("Duration: {}\n", Self::format_duration(duration)));
            }
        }

        content.push('\n');

        if self.show_details {
            content.push_str("## Suite Results\n\n");

            for suite_result in &run_result.suite_results {
                let status = if suite_result.all_passed() {
                    "✓ PASS"
                } else {
                    "✗ FAIL"
                };

                content.push_str(&format!("{} {}", status, suite_result.name));

                if self.show_timing {
                    if let Some(ref duration) = suite_result.duration {
                        content.push_str(&format!(" ({})", Self::format_duration(duration)));
                    }
                }

                content.push('\n');

                for test_result in &suite_result.test_results {
                    let test_status = match test_result.status {
                        crate::TestStatus::Passed => "  ✓",
                        crate::TestStatus::Failed => "  ✗",
                        crate::TestStatus::Skipped => "  -",
                        crate::TestStatus::Ignored => "  ⊝",
                        _ => "  ?",
                    };

                    content.push_str(&format!("{} {}", test_status, test_result.name));

                    if self.show_timing {
                        if let Some(ref duration) = test_result.duration {
                            content.push_str(&format!(" ({})", Self::format_duration(duration)));
                        }
                    }

                    content.push('\n');

                    if let Some(ref error) = test_result.error {
                        content.push_str(&format!("    Error: {}\n", error));
                    }
                }

                content.push('\n');
            }
        }

        let overall_status = if run_result.all_passed() {
            "All tests passed!"
        } else {
            "Some tests failed."
        };

        content.push_str(&format!("\n{}\n", overall_status));

        Ok(TestReport {
            metadata: self.metadata.clone(),
            run_result: run_result.clone(),
            format: ReportFormat::Text,
            content,
            created_at: Utc::now(),
        })
    }

    fn format(&self) -> ReportFormat {
        ReportFormat::Text
    }
}
