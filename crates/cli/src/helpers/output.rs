use colored::*;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde_json;
use sheila::format_relative_path;
use std::fmt::Write;
use std::time::Duration;
use tiny_gradient::{Gradient, GradientStr};

use crate::cli::OutputFormat;
use crate::discovery::TestFile;
use crate::helpers::tag_color;

pub struct OutputFormatter;

impl OutputFormatter {
    pub fn format_header(title: &str, gradient: Gradient) -> String {
        let separator_line = "=".repeat(60);
        let separator = separator_line.gradient(gradient);
        let title_colored = title.gradient(gradient);
        format!(
            "\n{}\n{}{}{}\n{}\n",
            separator,
            " ".repeat(30 - title.len() / 2),
            title_colored,
            " ".repeat(30 - title.len() / 2),
            separator
        )
    }

    pub fn format_success(message: &str) -> String {
        format!("{} {}\n", "✓".bright_green().bold(), message.bright_green())
    }

    pub fn format_error(message: &str) -> String {
        format!("{} {}", "✗".bright_red().bold(), message.bright_red())
    }

    pub fn format_warning(message: &str) -> String {
        format!("{} {}", "⚠".yellow().bold(), message.yellow())
    }

    pub fn format_info(message: &str) -> String {
        format!("{} {}", "ℹ".blue().bold(), message.bright_white())
    }

    pub fn format_progress(message: &str) -> String {
        format!("{} {}", "⏳".cyan(), message.cyan())
    }

    pub fn create_multi_progress(
        message: &str,
        total: Option<u64>,
        spinner: bool,
    ) -> (MultiProgress, ProgressBar) {
        let mb = MultiProgress::new();
        let pb = if spinner {
            Self::create_spinner(Some(message))
        } else {
            Self::create_progress_bar(message, total)
        };

        mb.add(pb.clone());

        (mb, pb)
    }

    pub fn create_spinner(message: Option<&str>) -> ProgressBar {
        let pb = ProgressBar::new(200);
        pb.set_style(
            ProgressStyle::with_template("{msg:>12.dim.bold} {spinner:.green} {msg:>12.dim.bold}")
                .unwrap(),
        );
        if let Some(message) = message {
            pb.set_message(message.to_string());
        }
        pb
    }

    pub fn create_progress_bar(message: &str, total: Option<u64>) -> ProgressBar {
        let pb = ProgressBar::new(total.unwrap_or(200));

        match total {
            Some(_) => {
                pb.set_style(
                    ProgressStyle::with_template(
                        "[{pos}/{len}] {msg:>12.dim.bold} {bar:100.white/dim}",
                    )
                    .unwrap()
                    .progress_chars("█▓▒░  "),
                );
            }
            None => {
                pb.set_style(
                    ProgressStyle::with_template("{msg:>12.dim.bold} {bar:100.white/dim}")
                        .unwrap()
                        .progress_chars("█▓▒░  "),
                );
            }
        }

        pb.set_message(message.to_string());

        pb
    }

    pub fn format_test_summary(
        passed: usize,
        failed: usize,
        total: usize,
        duration: Duration,
    ) -> String {
        let status_color = if failed == 0 { "green" } else { "red" };
        let duration_str = Self::format_duration(duration);

        format!(
            "\n{}\n  {} passed\n  {} failed\n  {} total\n  Time: {}\n",
            "Summary:".bright_white().bold(),
            passed.to_string().color(status_color).bold(),
            failed
                .to_string()
                .color(if failed == 0 { "dimmed" } else { "red" })
                .bold(),
            total.to_string().bright_white().bold(),
            duration_str.bright_white()
        )
    }

    pub fn format_duration(duration: Duration) -> String {
        let millis = duration.as_millis();
        if millis < 1000 {
            format!("{}ms", millis)
        } else if millis < 60_000 {
            format!("{:.2}s", duration.as_secs_f64())
        } else {
            let mins = duration.as_secs() / 60;
            let secs = duration.as_secs() % 60;
            format!("{}m {}s", mins, secs)
        }
    }

    pub fn format_abridged_summary(
        passed: usize,
        failed: usize,
        total: usize,
        duration: Duration,
    ) -> String {
        let duration_str = Self::format_duration(duration);
        format!(
            "\n{} {} {} {} {} {} {}\n",
            format!("✓ {}", passed).bright_green().bold(),
            "passed,".dimmed(),
            format!("✗ {}", failed).red().bold(),
            "failed,".dimmed(),
            format!("{}", total).bright_white().bold(),
            "total".dimmed(),
            format!("({} elapsed)", duration_str).dimmed().italic(),
        )
    }

    pub fn format_test_files(files: &[TestFile], format: OutputFormat) -> anyhow::Result<String> {
        match format {
            OutputFormat::Json => Self::format_json(files),
            OutputFormat::Csv => Self::format_csv(files),
            OutputFormat::Html => Self::format_html(files),
            OutputFormat::Text => Ok(Self::format_text(files)),
            _ => anyhow::bail!("Unsupported output format: {}", format),
        }
    }

    fn format_json(files: &[TestFile]) -> anyhow::Result<String> {
        serde_json::to_string_pretty(files).map_err(Into::into)
    }

    fn format_csv(files: &[TestFile]) -> anyhow::Result<String> {
        let mut output = String::new();
        writeln!(
            output,
            "file_path,suite_name,test_name,line_number,tags,ignored"
        )?;

        for file in files {
            for suite in &file.suites {
                for test in &suite.tests {
                    writeln!(
                        output,
                        "\"{}\",\"{}\",\"{}\",{},\"{}\",{}",
                        file.path.display(),
                        suite.name,
                        test.name,
                        test.line_number.unwrap_or(0),
                        test.tags.join(";"),
                        test.ignored
                    )?;
                }
            }
        }

        Ok(output)
    }

    fn format_html(files: &[TestFile]) -> anyhow::Result<String> {
        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        html.push_str("<title>Sheila Test Discovery</title>\n");
        html.push_str("<style>\n");
        html.push_str("body { font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; margin: 20px; }\n");
        html.push_str(
            ".file { margin-bottom: 30px; border: 1px solid #ddd; border-radius: 8px; }\n",
        );
        html.push_str(
            ".file-header { background: #f8f9fa; padding: 15px; border-bottom: 1px solid #ddd; }\n",
        );
        html.push_str(".suite { margin: 15px; }\n");
        html.push_str(".test { margin-left: 20px; padding: 5px 0; }\n");
        html.push_str(".test.ignored { opacity: 0.6; }\n");
        html.push_str(".tag { background: #e9ecef; padding: 2px 6px; border-radius: 3px; font-size: 0.8em; margin-left: 5px; }\n");
        html.push_str("</style>\n</head>\n<body>\n");

        html.push_str("<h1>Test Discovery Results</h1>\n");

        for file in files {
            html.push_str(&format!("<div class=\"file\">\n"));
            html.push_str(&format!(
                "<div class=\"file-header\"><h2>{}</h2></div>\n",
                file.path.display()
            ));

            for suite in &file.suites {
                html.push_str(&format!("<div class=\"suite\">\n"));
                html.push_str(&format!("<h3>● {}</h3>\n", suite.name));

                if suite.tests.is_empty() {
                    html.push_str("<p><em>No tests in this suite</em></p>\n");
                } else {
                    for test in &suite.tests {
                        let ignored_class = if test.ignored { " ignored" } else { "" };
                        html.push_str(&format!("<div class=\"test{}\">\n", ignored_class));
                        let icon = if test.ignored { "○" } else { "✓" };
                        html.push_str(&format!(
                            "{} {} [line {}]",
                            icon,
                            test.name,
                            test.line_number.unwrap_or(0)
                        ));

                        for tag in &test.tags {
                            html.push_str(&format!("<span class=\"tag\">{}</span>", tag));
                        }

                        html.push_str("</div>\n");
                    }
                }
                html.push_str("</div>\n");
            }
            html.push_str("</div>\n");
        }

        html.push_str("</body>\n</html>\n");
        Ok(html)
    }

    fn format_text(files: &[TestFile]) -> String {
        let mut output = String::new();
        let mut total_suites = 0;
        let mut total_tests = 0;
        let mut ignored_tests = 0;

        output.push_str("\n\n");

        for file in files {
            output.push_str(&format!(
                "{}\n",
                format_relative_path(&file.path).gradient(Gradient::Cristal)
            ));

            for suite in &file.suites {
                total_suites += 1;
                output.push_str(&format!(
                    "  {} {}\n",
                    "●".bright_blue(),
                    suite.name.bright_white()
                ));

                if suite.tests.is_empty() {
                    output.push_str(&format!("    {}\n", "No tests in this suite".dimmed()));
                } else {
                    for test in &suite.tests {
                        total_tests += 1;
                        let icon = if test.ignored {
                            ignored_tests += 1;
                            "○".yellow()
                        } else {
                            "✓".green()
                        };

                        let mut test_line = format!("    {} {}", icon, test.name);

                        if let Some(line_num) = test.line_number {
                            test_line
                                .push_str(&format!(" {}", format!("[line {}]", line_num).dimmed()));
                        }

                        if !test.tags.is_empty() {
                            for tag in &test.tags {
                                let tag_clr = format!(
                                    "@{}",
                                    tag.replace("\"", "").replace("[", "").replace("]", "")
                                );
                                test_line.push_str(&format!(
                                    " {}",
                                    tag_clr.color(tag_color(tag_clr.clone()))
                                ));
                            }
                        }

                        let mut attributes = Vec::new();
                        if let Some(timeout) = test.timeout {
                            attributes.push(format!("timeout {}s", timeout));
                        }
                        if let Some(retries) = test.retries {
                            attributes.push(format!("retries {}", retries));
                        }

                        if !attributes.is_empty() {
                            test_line.push_str(&format!(
                                " {}",
                                format!("[{}]", attributes.join(", ")).dimmed()
                            ));
                        }

                        output.push_str(&format!("{}\n", test_line));
                    }
                }
            }
            output.push('\n');
        }

        let active_tests = total_tests - ignored_tests;
        output.push_str(&format!(
            "{}\n\n",
            format!(
                "Found {} files, {} test suites, {} tests ({} ignored)",
                files.len(),
                total_suites,
                active_tests,
                ignored_tests
            )
            .bright_white()
            .bold()
        ));

        output
    }
}
