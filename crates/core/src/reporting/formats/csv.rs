use super::*;

/// Builtin reporter that generates reports in CSV format
///
/// This reporter requires the `csv` or `reporters` feature to be enabled.
pub struct CsvReporter {
    metadata: ReportMetadata,
    include_headers: bool,
}

impl CsvReporter {
    pub fn new() -> Self {
        Self {
            metadata: ReportMetadata::default(),
            include_headers: true,
        }
    }

    pub fn with_metadata(mut self, metadata: ReportMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn include_headers(mut self, include: bool) -> Self {
        self.include_headers = include;
        self
    }
}

impl Default for CsvReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for CsvReporter {
    fn generate(&self, run_result: &RunResult) -> Result<TestReport> {
        let mut content = String::new();

        if self.include_headers {
            content.push_str("suite_name,test_name,status,duration_ms,error\n");
        }

        for suite_result in &run_result.suite_results {
            for test_result in &suite_result.test_results {
                let duration_ms = test_result
                    .duration
                    .map(|d| d.as_millis().to_string())
                    .unwrap_or_else(|| "".to_string());

                let error = test_result
                    .error
                    .as_ref()
                    .map(|e| format!("\"{}\"", e.to_string().replace('"', "\"\"")))
                    .unwrap_or_else(|| "".to_string());

                content.push_str(&format!(
                    "\"{}\",\"{}\",{},{},{}\n",
                    suite_result.name, test_result.name, test_result.status, duration_ms, error
                ));
            }
        }

        Ok(TestReport {
            metadata: self.metadata.clone(),
            run_result: run_result.clone(),
            format: ReportFormat::Csv,
            content,
            created_at: Utc::now(),
        })
    }

    fn format(&self) -> ReportFormat {
        ReportFormat::Csv
    }
}
