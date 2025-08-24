use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::runners::RunResult;

/// The format of the report to be generated.
///
/// The `Composite` format is a special format that allows for the
/// generation of a report that is a composite of multiple of the
/// above formats.
///
/// Text and Composite report formats are enabled by default. For other formats,
/// you'll need to enable the appropriate feature, or used the `reporters` feature
/// to enable all formats (and their corresponding reporters).
///
/// Builtin formats:
///
/// - `Text`: Plain text
/// - `Json`: Structured JSON (requires `json` feature)
/// - `Csv`: Comma-delimited spreadsheet (requires `csv` feature)
/// - `Html`: HTML page (requires `html` feature)
/// - `JUnit`: JUnit XML (requires `junit` feature)
/// - `Tap`: Test Anything Protocol (requires `tap` feature)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportFormat {
    Text,
    #[cfg(feature = "json")]
    Json,
    #[cfg(feature = "csv")]
    Csv,
    #[cfg(feature = "html")]
    Html,
    #[cfg(feature = "junit")]
    JUnit,
    #[cfg(feature = "tap")]
    Tap,
    Composite(Vec<ReportFormat>),
}

impl fmt::Display for ReportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReportFormat::Text => write!(f, "text"),
            #[cfg(feature = "json")]
            ReportFormat::Json => write!(f, "json"),
            #[cfg(feature = "csv")]
            ReportFormat::Csv => write!(f, "csv"),
            #[cfg(feature = "html")]
            ReportFormat::Html => write!(f, "html"),
            #[cfg(feature = "junit")]
            ReportFormat::JUnit => write!(f, "junit"),
            #[cfg(feature = "tap")]
            ReportFormat::Tap => write!(f, "tap"),
            ReportFormat::Composite(formats) => write!(
                f,
                "composite({})",
                formats
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    pub metadata: ReportMetadata,
    pub run_result: RunResult,
    pub format: ReportFormat,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    pub title: String,
    pub description: Option<String>,
    pub version: String,
    pub generator: String,
    pub custom: HashMap<String, String>,
}

impl Default for ReportMetadata {
    fn default() -> Self {
        Self {
            title: "Test Report".to_string(),
            description: None,
            version: "1.0".to_string(),
            generator: "Sheila Testing Framework".to_string(),
            custom: HashMap::new(),
        }
    }
}
