use crate::cli::{OutputFormat, ReportArgs};
use crate::helpers::OutputFormatter;
use crate::helpers::{format_duration, get_default_output_dir, get_most_recent_report};
use sheila::runners::RunResult;
use sheila::{ReportFormat, TestReport};
use std::fs;
use std::path::{Path, PathBuf};
use tiny_gradient::{Gradient, GradientStr};

use colored::*;

pub async fn run(mut args: ReportArgs) -> color_eyre::Result<()> {
    let report_path = if let Some(path) = args.path.take() {
        path
    } else {
        let output_dir = get_default_output_dir().unwrap_or_else(|_| PathBuf::from("."));

        get_most_recent_report(&output_dir)
            .map_err(|_| {
                sheila::Error::generic("No reports found. Run tests first to generate a report.")
            })?
            .ok_or_else(|| {
                sheila::Error::generic("No reports found. Run tests first to generate a report.")
            })?
    };

    if !report_path.exists() {
        return Err(sheila::Error::generic(format!(
            "Report file not found: {}",
            report_path.display()
        ))
        .into());
    }

    println!(
        "{}",
        OutputFormatter::format_info(&format!("Reading report from: {}", report_path.display()))
    );

    let content = fs::read_to_string(&report_path).map_err(|_| {
        sheila::Error::generic(format!(
            "Failed to read report file: {}",
            report_path.display()
        ))
    })?;

    let file_format = detect_file_format(&report_path)?;

    match file_format {
        ReportFormat::Json => display_json_report(&content, &args).await,
        ReportFormat::Csv => display_csv_report(&content, &args).await,
        ReportFormat::Html => display_html_report(&content, &args).await,
        ReportFormat::Text => display_text_report(&content, &args).await,
        _ => {
            return Err(sheila::Error::generic(format!(
                "Unsupported report format: {}",
                file_format
            ))
            .into());
        }
    }
}

fn detect_file_format(path: &Path) -> color_eyre::Result<ReportFormat> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => Ok(ReportFormat::Json),
        Some("csv") => Ok(ReportFormat::Csv),
        Some("html") | Some("htm") => Ok(ReportFormat::Html),
        Some("txt") => Ok(ReportFormat::Text),
        _ => Ok(ReportFormat::Json),
    }
}

async fn display_json_report(content: &str, args: &ReportArgs) -> color_eyre::Result<()> {
    if let Ok(test_report) = serde_json::from_str::<TestReport>(content) {
        display_run_result(&test_report.run_result, args).await
    } else if let Ok(run_result) = serde_json::from_str::<RunResult>(content) {
        display_run_result(&run_result, args).await
    } else {
        match args.format.unwrap_or(OutputFormat::Text) {
            OutputFormat::Json => {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(content) {
                    println!("{}", serde_json::to_string_pretty(&value)?);
                } else {
                    println!("{}", content);
                }
            }
            _ => {
                println!(
                    "{}",
                    OutputFormatter::format_warning(
                        "Could not parse JSON report as TestReport or RunResult"
                    )
                );
                println!("{}", content);
            }
        }
        Ok(())
    }
}

async fn display_csv_report(content: &str, args: &ReportArgs) -> color_eyre::Result<()> {
    match args.format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Text => display_csv_as_table(content, args),
        OutputFormat::Csv => {
            println!("{}", content);
            Ok(())
        }
        OutputFormat::Json => {
            let json = csv_to_json(content)?;
            println!("{}", serde_json::to_string_pretty(&json)?);
            Ok(())
        }
        OutputFormat::Html => {
            let html = csv_to_html(content)?;
            println!("{}", html);
            Ok(())
        }
        _ => {
            return Err(sheila::Error::generic(format!(
                "Unsupported report format: {}",
                args.format.unwrap()
            ))
            .into());
        }
    }
}

async fn display_html_report(content: &str, args: &ReportArgs) -> color_eyre::Result<()> {
    match args.format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Html => {
            println!("{}", content);
        }
        OutputFormat::Text => {
            println!(
                "{}",
                OutputFormatter::format_info(
                    "HTML report detected. Use --format html to display raw HTML, or open in a browser:"
                )
            );
            println!(
                "  {}",
                std::env::current_dir()?.join("report.html").display()
            );

            let text_content = html_to_text(content);
            println!("\nExtracted content:");
            println!("{}", text_content);
        }
        _ => {
            println!(
                "{}",
                OutputFormatter::format_warning("Cannot convert HTML to the requested format")
            );
            println!("{}", content);
        }
    }
    Ok(())
}

async fn display_text_report(content: &str, _args: &ReportArgs) -> color_eyre::Result<()> {
    println!("{}", content);
    Ok(())
}

async fn display_run_result(run_result: &RunResult, args: &ReportArgs) -> color_eyre::Result<()> {
    let passed = run_result.passed_tests;
    let failed = run_result.failed_tests;
    let ignored = run_result.skipped_tests;
    let total = run_result.total_tests;

    println!();

    println!("{}", "=".repeat(60).gradient(Gradient::Passion));
    println!(
        "{}",
        format!(
            "{} {}",
            "TEST REPORT -",
            run_result.start_time.format("%Y-%m-%d %H:%M:%S UTC")
        )
        .gradient(Gradient::Passion)
    );
    println!("{}", "=".repeat(60).gradient(Gradient::Passion));

    println!();
    println!(
        "  {} {} {}",
        "√".green(),
        passed.to_string().green(),
        "passed".dimmed()
    );
    if failed > 0 {
        println!(
            "  {} {} {}",
            "✗".red(),
            failed.to_string().red(),
            "failed".dimmed()
        );
    }
    if ignored > 0 {
        println!(
            "  {} {} {}",
            "○".yellow(),
            ignored.to_string().yellow(),
            "ignored".dimmed()
        );
    }
    println!(
        "  {} {} {}",
        "∑".blue(),
        total.to_string().bright_white().bold(),
        "total".dimmed().bold()
    );

    if args.verbose || args.failures_only {
        println!("\n{}", "Detailed Results:".bright_white());

        for suite_result in &run_result.suite_results {
            let should_show_suite = !args.failures_only || !suite_result.all_passed();

            if should_show_suite {
                println!(
                    "\n{} {}",
                    "●".bright_blue(),
                    suite_result.name.bright_white()
                );

                for test_result in &suite_result.test_results {
                    let should_show_test =
                        !args.failures_only || test_result.status == sheila::TestStatus::Failed;

                    if should_show_test {
                        let status_icon = match test_result.status {
                            sheila::TestStatus::Passed => "✓".green(),
                            sheila::TestStatus::Failed => "✗".red(),
                            sheila::TestStatus::Ignored => "○".yellow(),
                            _ => "?".dimmed(),
                        };

                        println!("  {} {}", status_icon, test_result.name);

                        if args.verbose {
                            if let Some(duration) = test_result.duration {
                                println!("    Duration: {}", format_duration(duration).dimmed());
                            }
                        }

                        if test_result.status == sheila::TestStatus::Failed {
                            if let Some(error) = &test_result.error {
                                println!("    {}", format!("Error: {}", error).red());
                            }
                        }
                    }
                }
            }
        }
    }

    println!();
    if run_result.all_passed() {
        println!("{}", OutputFormatter::format_success("All tests passed!"));
    } else {
        println!(
            "{}",
            OutputFormatter::format_error(&format!("{} test(s) failed", failed))
        );
    }

    Ok(())
}

fn display_csv_as_table(content: &str, args: &ReportArgs) -> color_eyre::Result<()> {
    let mut reader = csv::Reader::from_reader(content.as_bytes());
    let headers = reader.headers()?.clone();

    println!(
        "{}",
        headers
            .iter()
            .collect::<Vec<_>>()
            .join(" | ")
            .bright_white()
    );
    println!("{}", "-".repeat(headers.len() * 20));

    for result in reader.records() {
        let record = result?;
        let mut row_data = Vec::new();

        for (i, field) in record.iter().enumerate() {
            if args.failures_only && i == 2 {
                if field != "Failed" && field != "failed" {
                    continue;
                }
            }
            row_data.push(field);
        }

        if !args.failures_only || row_data.len() > 0 {
            println!("{}", row_data.join(" | "));
        }
    }

    Ok(())
}

fn csv_to_json(content: &str) -> color_eyre::Result<serde_json::Value> {
    let mut reader = csv::Reader::from_reader(content.as_bytes());
    let headers = reader.headers()?.clone();
    let mut records = Vec::new();

    for result in reader.records() {
        let record = result?;
        let mut map = serde_json::Map::new();

        for (header, field) in headers.iter().zip(record.iter()) {
            map.insert(
                header.to_string(),
                serde_json::Value::String(field.to_string()),
            );
        }

        records.push(serde_json::Value::Object(map));
    }

    Ok(serde_json::Value::Array(records))
}

fn csv_to_html(content: &str) -> color_eyre::Result<String> {
    let mut reader = csv::Reader::from_reader(content.as_bytes());
    let headers = reader.headers()?.clone();
    let mut html = String::from("<table border=\"1\">\n<thead>\n<tr>\n");

    for header in headers.iter() {
        html.push_str(&format!("<th>{}</th>\n", header));
    }
    html.push_str("</tr>\n</thead>\n<tbody>\n");

    for result in reader.records() {
        let record = result?;
        html.push_str("<tr>\n");
        for field in record.iter() {
            html.push_str(&format!("<td>{}</td>\n", field));
        }
        html.push_str("</tr>\n");
    }

    html.push_str("</tbody>\n</table>");
    Ok(html)
}

fn html_to_text(html: &str) -> String {
    html.replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("</p>", "\n")
        .replace("</div>", "\n")
        .replace("</h1>", "\n")
        .replace("</h2>", "\n")
        .replace("</h3>", "\n")
        .replace("</li>", "\n")
        .chars()
        .fold((String::new(), false), |(mut acc, in_tag), ch| match ch {
            '<' => (acc, true),
            '>' => (acc, false),
            c if !in_tag => {
                acc.push(c);
                (acc, false)
            }
            _ => (acc, in_tag),
        })
        .0
}
