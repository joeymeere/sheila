use super::*;

/// Builtin reporter that generates reports in HTML format
///
/// This reporter requires the `html` or `reporters` feature to be enabled.
pub struct HtmlReporter {
    metadata: ReportMetadata,
    include_styles: bool,
    show_timing: bool,
}

impl HtmlReporter {
    pub fn new() -> Self {
        Self {
            metadata: ReportMetadata::default(),
            include_styles: true,
            show_timing: true,
        }
    }

    pub fn with_metadata(mut self, metadata: ReportMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn include_styles(mut self, include: bool) -> Self {
        self.include_styles = include;
        self
    }

    /// Show information about execution time for each test,
    /// each suite, and the total duration of the run.
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

    fn generate_styles(&self) -> String {
        if !self.include_styles {
            return String::new();
        }

        r#"<style>
            body { 
                font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; 
                margin: 20px; 
                background: #f8f9fa; 
            }
            .header { 
                background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                color: white;
                padding: 30px;
                border-radius: 10px;
                margin-bottom: 30px;
                text-align: center;
            }
            .header h1 { margin: 0; font-size: 2.5em; }
            .header .subtitle { opacity: 0.9; margin-top: 10px; }
            .summary { 
                display: grid; 
                grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); 
                gap: 20px; 
                margin-bottom: 30px; 
            }
            .summary-card { 
                background: white; 
                padding: 20px; 
                border-radius: 8px; 
                box-shadow: 0 2px 4px rgba(0,0,0,0.1); 
                text-align: center;
            }
            .summary-card .number { font-size: 2em; font-weight: bold; margin-bottom: 5px; }
            .summary-card .label { color: #666; text-transform: uppercase; font-size: 0.8em; }
            .passed { color: #28a745; }
            .failed { color: #dc3545; }
            .skipped { color: #ffc107; }
            .suite { 
                background: white; 
                margin-bottom: 20px; 
                border-radius: 8px; 
                overflow: hidden;
                box-shadow: 0 2px 4px rgba(0,0,0,0.1); 
            }
            .suite-header { 
                padding: 15px 20px; 
                border-bottom: 1px solid #dee2e6; 
                display: flex; 
                justify-content: space-between; 
                align-items: center;
            }
            .suite-header.passed { background: #d4edda; }
            .suite-header.failed { background: #f8d7da; }
            .suite-title { font-weight: bold; font-size: 1.1em; }
            .test { 
                padding: 10px 20px; 
                border-bottom: 1px solid #f8f9fa; 
                display: flex; 
                justify-content: space-between; 
                align-items: center;
            }
            .test:last-child { border-bottom: none; }
            .test-name { display: flex; align-items: center; }
            .test-status { margin-right: 10px; font-weight: bold; }
            .test-details { color: #666; font-size: 0.9em; }
            .error { 
                background: #f8f9fa; 
                padding: 10px 20px; 
                font-family: 'Courier New', monospace; 
                font-size: 0.9em; 
                color: #dc3545;
                white-space: pre-wrap;
            }
            .footer {
                text-align: center;
                margin-top: 40px;
                padding: 20px;
                color: #666;
                font-size: 0.9em;
            }
        </style>"#
            .to_string()
    }
}

impl Default for HtmlReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for HtmlReporter {
    fn generate(&self, run_result: &RunResult) -> Result<TestReport> {
        let mut html = String::new();

        html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        html.push_str("<meta charset=\"UTF-8\">\n");
        html.push_str(
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n",
        );
        html.push_str(&format!("<title>{}</title>\n", self.metadata.title));
        html.push_str(&self.generate_styles());
        html.push_str("</head>\n<body>\n");

        html.push_str("<div class=\"header\">\n");
        html.push_str(&format!("<h1>{}</h1>\n", self.metadata.title));
        if let Some(ref description) = self.metadata.description {
            html.push_str(&format!("<div class=\"subtitle\">{}</div>\n", description));
        }
        html.push_str(&format!(
            "<div class=\"subtitle\">Generated on {}</div>\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));
        html.push_str("</div>\n");

        html.push_str("<div class=\"summary\">\n");

        html.push_str("<div class=\"summary-card\">\n");
        html.push_str(&format!(
            "<div class=\"number passed\">{}</div>\n",
            run_result.passed_tests
        ));
        html.push_str("<div class=\"label\">Passed</div>\n");
        html.push_str("</div>\n");

        html.push_str("<div class=\"summary-card\">\n");
        html.push_str(&format!(
            "<div class=\"number failed\">{}</div>\n",
            run_result.failed_tests
        ));
        html.push_str("<div class=\"label\">Failed</div>\n");
        html.push_str("</div>\n");

        html.push_str("<div class=\"summary-card\">\n");
        html.push_str(&format!(
            "<div class=\"number skipped\">{}</div>\n",
            run_result.skipped_tests
        ));
        html.push_str("<div class=\"label\">Skipped</div>\n");
        html.push_str("</div>\n");

        html.push_str("<div class=\"summary-card\">\n");
        html.push_str(&format!(
            "<div class=\"number\">{}</div>\n",
            run_result.total_tests
        ));
        html.push_str("<div class=\"label\">Total</div>\n");
        html.push_str("</div>\n");

        if let Some(ref duration) = run_result.duration {
            html.push_str("<div class=\"summary-card\">\n");
            html.push_str(&format!(
                "<div class=\"number\">{}</div>\n",
                Self::format_duration(duration)
            ));
            html.push_str("<div class=\"label\">Duration</div>\n");
            html.push_str("</div>\n");
        }

        html.push_str("</div>\n");

        for suite_result in &run_result.suite_results {
            let suite_class = if suite_result.all_passed() {
                "passed"
            } else {
                "failed"
            };

            html.push_str("<div class=\"suite\">\n");
            html.push_str(&format!("<div class=\"suite-header {}\">\n", suite_class));
            html.push_str("<div class=\"suite-title\">");
            html.push_str(&format!(
                "{} {}",
                if suite_result.all_passed() {
                    "✓"
                } else {
                    "✗"
                },
                suite_result.name
            ));
            html.push_str("</div>\n");

            if self.show_timing {
                if let Some(ref duration) = suite_result.duration {
                    html.push_str(&format!(
                        "<div class=\"test-details\">{}</div>\n",
                        Self::format_duration(duration)
                    ));
                }
            }

            html.push_str("</div>\n");

            for test_result in &suite_result.test_results {
                html.push_str("<div class=\"test\">\n");
                html.push_str("<div class=\"test-name\">\n");

                let (icon, class) = match test_result.status {
                    crate::TestStatus::Passed => ("✓", "passed"),
                    crate::TestStatus::Failed => ("✗", "failed"),
                    crate::TestStatus::Skipped => ("○", "skipped"),
                    crate::TestStatus::Ignored => ("⊝", "skipped"),
                    _ => ("?", ""),
                };

                html.push_str(&format!(
                    "<span class=\"test-status {}\">{}</span>\n",
                    class, icon
                ));
                html.push_str(&format!("<span>{}</span>\n", test_result.name));
                html.push_str("</div>\n");

                if self.show_timing {
                    if let Some(ref duration) = test_result.duration {
                        html.push_str(&format!(
                            "<div class=\"test-details\">{}</div>\n",
                            Self::format_duration(duration)
                        ));
                    }
                }

                html.push_str("</div>\n");

                if let Some(ref error) = test_result.error {
                    html.push_str(&format!(
                        "<div class=\"error\">{}</div>\n",
                        html_escape::encode_text(&error.to_string())
                    ));
                }
            }

            html.push_str("</div>\n");
        }

        html.push_str("<div class=\"footer\">\n");
        html.push_str(&format!(
            "Generated by {} v{}\n",
            self.metadata.generator, self.metadata.version
        ));
        html.push_str("</div>\n");

        html.push_str("</body>\n</html>\n");

        Ok(TestReport {
            metadata: self.metadata.clone(),
            run_result: run_result.clone(),
            format: ReportFormat::Html,
            content: html,
            created_at: Utc::now(),
        })
    }

    fn format(&self) -> ReportFormat {
        ReportFormat::Html
    }
}
