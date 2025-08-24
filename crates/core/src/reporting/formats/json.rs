use super::*;

pub struct JsonReporter {
    metadata: ReportMetadata,
    pretty: bool,
}

impl JsonReporter {
    pub fn new() -> Self {
        Self {
            metadata: ReportMetadata::default(),
            pretty: true,
        }
    }

    pub fn with_metadata(mut self, metadata: ReportMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn pretty(mut self, pretty: bool) -> Self {
        self.pretty = pretty;
        self
    }
}

impl Default for JsonReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for JsonReporter {
    fn generate(&self, run_result: &RunResult) -> Result<TestReport> {
        let content = if self.pretty {
            serde_json::to_string_pretty(run_result)?
        } else {
            serde_json::to_string(run_result)?
        };

        Ok(TestReport {
            metadata: self.metadata.clone(),
            run_result: run_result.clone(),
            format: ReportFormat::Json,
            content,
            created_at: Utc::now(),
        })
    }

    fn format(&self) -> ReportFormat {
        ReportFormat::Json
    }
}
