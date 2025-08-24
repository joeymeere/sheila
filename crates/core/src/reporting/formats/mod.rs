#[cfg(feature = "csv")]
pub mod csv;
#[cfg(feature = "csv")]
pub use csv::*;

#[cfg(feature = "html")]
pub mod html;
#[cfg(feature = "html")]
pub use html::*;

#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "json")]
pub use json::*;

use super::*;

/// Builtin reporter that generates multiple reports from multiple specified reporters.
///
/// This reporter is enabled by default.
pub struct CompositeReporter {
    reporters: Vec<Box<dyn Reporter>>,
}

impl CompositeReporter {
    pub fn new() -> Self {
        Self {
            reporters: Vec::new(),
        }
    }

    pub fn add_reporter(mut self, reporter: Box<dyn Reporter>) -> Self {
        self.reporters.push(reporter);
        self
    }

    pub fn generate_all(&self, run_result: &RunResult) -> Result<Vec<TestReport>> {
        let mut reports = Vec::new();

        for reporter in &self.reporters {
            reports.push(reporter.generate(run_result)?);
        }

        Ok(reports)
    }
}

impl Default for CompositeReporter {
    fn default() -> Self {
        Self::new()
    }
}
