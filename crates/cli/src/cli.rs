use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sheila")]
#[command(about = "Run, debug, and view results of sheila tests")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run tests according to the specified inputs
    Test(TestArgs),
    /// List all available test suites and their tests
    List(ListArgs),
    /// Pretty print a JSON or CSV report
    Report(ReportArgs),
    /// Stop a headless test running in the background
    Stop(ControlArgs),
    /// Pause a headless test running in the background
    Pause(ControlArgs),
    /// Resume a previously paused headless test running in the background
    Resume(ControlArgs),
    /// Clear all caches
    #[command(name = "clear-cache")]
    ClearCache,
}

#[derive(Parser)]
pub struct TestArgs {
    /// Path to test file, test file with line number, test function name, or test tag
    pub target: Option<String>,

    /// Run tests in headless mode (background) and return an ID
    #[arg(long = "headless")]
    pub headless: bool,

    /// Show debug logs from tests/test runner
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,

    /// Output format for test results
    #[arg(short, long, value_enum)]
    pub output: Option<OutputFormat>,

    /// Run tests matching the given grep expression
    #[arg(short, long)]
    pub grep: Option<String>,

    /// Maximum number of concurrent test suites
    #[arg(long)]
    pub max_concurrent: Option<usize>,

    /// Stop on first failure
    #[arg(long)]
    pub fail_fast: bool,

    /// Stream test output
    #[arg(long, default_value_t = true)]
    pub stream: bool,

    /// Test timeout in seconds
    #[arg(long)]
    pub timeout: Option<u64>,

    /// Include tests with specific tags
    #[arg(long, value_delimiter = ',')]
    pub tags: Vec<String>,

    /// Exclude tests with specific tags
    #[arg(long, value_delimiter = ',')]
    pub exclude_tags: Vec<String>,

    /// Output directory for reports
    #[arg(long)]
    pub output_dir: Option<PathBuf>,
}

#[derive(Parser)]
pub struct ListArgs {
    /// Path to test file or directory to list tests from
    pub path: Option<PathBuf>,

    /// Show detailed information about each test
    #[arg(short, long)]
    pub verbose: bool,

    /// Filter tests by tag
    #[arg(long, value_delimiter = ',')]
    pub tags: Vec<String>,

    /// Output format for the list
    #[arg(short = 'f', long, value_enum, default_value = "text")]
    pub format: OutputFormat,
}

#[derive(Parser)]
pub struct ReportArgs {
    /// Path to the report file to display
    pub path: Option<PathBuf>,

    /// Output format for displaying the report
    #[arg(short, long, value_enum)]
    pub format: Option<OutputFormat>,

    /// Show only failed tests
    #[arg(long)]
    pub failures_only: bool,

    /// Show detailed test information
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Parser)]
pub struct ControlArgs {
    pub test_id: String,
}

/// Ditto of `ReportFormat` from the core crate -- needed
/// to impl `ValueEnum` and can't use tuple variants in clap
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Csv,
    Html,
    Junit,
    Tap,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Csv => write!(f, "csv"),
            OutputFormat::Html => write!(f, "html"),
            OutputFormat::Text => write!(f, "text"),
            OutputFormat::Junit => write!(f, "junit"),
            OutputFormat::Tap => write!(f, "tap"),
        }
    }
}
