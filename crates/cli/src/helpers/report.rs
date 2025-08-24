use std::time::Duration;

use chrono::Utc;
use colored::Colorize;
use sheila::{
    Reporter, TestStatus,
    reporting::{CsvReporter, HtmlReporter, JsonReporter, TextReporter},
    runners::RunResult,
};
use uuid::Uuid;

use crate::{
    cli::{OutputFormat, TestArgs},
    helpers::{OutputFormatter, ensure_dir_exists, get_default_output_dir, result_gradient},
};

pub fn display_test_results(
    result: &RunResult,
    args: &TestArgs,
    duration: Duration,
) -> color_eyre::Result<()> {
    let passed = result.passed_tests;
    let failed = result.failed_tests;
    let total = result.total_tests;

    println!(
        "{}",
        OutputFormatter::format_header("Test Results", result_gradient(passed, total))
    );

    if args.verbose {
        for suite_result in &result.suite_results {
            let suite_icon = if suite_result.all_passed() {
                "●"
            } else {
                "●"
            };
            let suite_color = if suite_result.all_passed() {
                "green"
            } else {
                "red"
            };

            println!(
                "{} {}",
                suite_icon.color(suite_color).bold(),
                suite_result.name.bright_white().bold()
            );

            for test_result in &suite_result.test_results {
                let (icon, color) = match test_result.status {
                    TestStatus::Passed => ("✓", "green"),
                    TestStatus::Failed => ("✗", "red"),
                    TestStatus::Skipped => ("○", "yellow"),
                    TestStatus::Ignored => ("⊝", "dimmed"),
                    _ => ("?", "white"),
                };

                println!("  {} {}", icon.color(color), test_result.name);

                if let Some(ref error) = test_result.error {
                    println!("    {}: {}", "Error".red(), error.to_string().dimmed());
                }
            }
            println!();
        }
    }

    println!(
        "{}",
        OutputFormatter::format_abridged_summary(passed, failed, total, duration)
    );

    if failed > 0 {
        println!("{}", OutputFormatter::format_error("Some tests failed"));
    } else {
        println!("{}", OutputFormatter::format_success("All tests passed!"));
    }

    Ok(())
}

pub fn generate_report(result: &RunResult, args: &TestArgs) -> color_eyre::Result<()> {
    let output_dir = args.output_dir.clone().unwrap_or_else(|| {
        get_default_output_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    });

    ensure_dir_exists(&output_dir)
        .map_err(|_| sheila::Error::generic("Failed to create output directory"))?;

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let report_id = Uuid::new_v4().to_string().replace('-', "")[..16].to_string();

    let reporter: Box<dyn Reporter> = match args.output.unwrap() {
        OutputFormat::Json => Box::new(JsonReporter::new()),
        OutputFormat::Csv => Box::new(CsvReporter::new()),
        OutputFormat::Html => Box::new(HtmlReporter::new()),
        OutputFormat::Text => Box::new(TextReporter::new()),
        OutputFormat::Junit => Box::new(TextReporter::new()),
        OutputFormat::Tap => Box::new(TextReporter::new()),
    };

    let report = reporter.generate(result)?;
    let filename = format!(
        "test_report_{}_{}.{}",
        timestamp,
        report_id,
        args.output.unwrap()
    );
    let report_path = output_dir.join(filename);

    std::fs::write(&report_path, &report.content)?;

    println!(
        "{}",
        OutputFormatter::format_success(&format!("Report generated: {}", report_path.display()))
    );

    Ok(())
}
