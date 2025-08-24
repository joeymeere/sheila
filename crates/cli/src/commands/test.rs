use crate::cli::{OutputFormat, TestArgs};
use crate::config::SheilaConfig;
use crate::discovery::{TestDiscovery, TestFile};
use crate::output::OutputFormatter;
use crate::utils::{TargetSpec, Utils};
use chrono::Utc;
use colored::*;
use indicatif::{MultiProgress, ProgressBar};
use sheila::reporting::{CsvReporter, HtmlReporter, JsonReporter, TextReporter};
use sheila::runners::{
    CargoRunnerConfig, CargoTestRunner, ProcessOutput, RunResult, TestExecutable,
};
use sheila::suite::SuiteResult;
use sheila::{Error, Reporter, RunnerConfig, TestStatus};
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{self, TryRecvError};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn determine_target_crate(file_path: &str) -> String {
    if file_path.contains("examples/") {
        "examples".to_string()
    } else if file_path.contains("crates/cli/") {
        "cli".to_string()
    } else if file_path.contains("crates/core/") {
        "core".to_string()
    } else if file_path.contains("crates/server/") {
        "server".to_string()
    } else if file_path.contains("crates/proc-macros/") {
        "proc_macros".to_string()
    } else {
        "examples".to_string()
    }
}

pub fn run(args: TestArgs) -> color_eyre::Result<()> {
    println!();

    if args.headless {
        println!(
            "{}",
            OutputFormatter::format_warning("Headless mode not yet implemented")
        );
        return Ok(());
    }

    let (mb, pb) = OutputFormatter::create_multi_progress("", None, true);
    let (args, filtered_files, total_tests) = run_discovery(args, &pb)?;

    pb.finish();
    mb.clear()?;
    mb.remove(&pb);

    let run_tests_pb =
        OutputFormatter::create_progress_bar("Running...", Some((total_tests + 1) as u64));
    run_tests_pb.set_prefix(format!("[0/{}]", total_tests));

    run_tests(args, filtered_files, &run_tests_pb, &mb, total_tests)?;

    Ok(())
}

fn run_discovery(
    args: TestArgs,
    pb: &ProgressBar,
) -> color_eyre::Result<(TestArgs, Vec<TestFile>, usize)> {
    let _config = SheilaConfig::load().map_err(|_| Error::generic("Failed to load config"))?;
    let discovery = TestDiscovery::new()?;
    let test_files = if let Some(target) = &args.target {
        let target_spec = Utils::parse_target(target);
        match target_spec {
            TargetSpec::File(file) => discovery.discover(Path::new(&file))?,
            TargetSpec::FileLine { file, .. } => discovery.discover(Path::new(&file))?,
            _ => discovery.discover_current()?,
        }
    } else {
        discovery.discover_current()?
    };

    let filtered_files = discovery.filter_tests(
        test_files,
        args.target.as_deref(),
        &args.tags,
        args.grep.as_deref(),
    )?;

    if filtered_files.is_empty() {
        pb.finish_with_message("No matching tests found.");
        return Ok((args, Vec::new(), 0));
    }

    let total_tests: usize = filtered_files
        .iter()
        .flat_map(|f| &f.suites)
        .map(|s| s.tests.len())
        .sum();

    Ok((args, filtered_files, total_tests))
}

fn run_tests(
    args: TestArgs,
    filtered_files: Vec<TestFile>,
    pb: &ProgressBar,
    mb: &MultiProgress,
    total_tests: usize,
) -> color_eyre::Result<()> {
    let mut runner_config = RunnerConfig::default();
    runner_config.fail_fast = args.fail_fast;

    if let Some(ref grep) = args.grep {
        runner_config.include_patterns.push(grep.clone());
    }

    runner_config.include_tags = args.tags.clone();

    let mut cargo_config = CargoRunnerConfig {
        stream_output: args.stream,
        ..Default::default()
    };

    if let Some(timeout) = args.timeout {
        cargo_config
            .test_args
            .push(format!("--timeout={}", timeout));
    }

    if args.verbose {
        cargo_config.test_args.push("--nocapture".to_string());
    }

    let (output_tx, output_rx) = mpsc::channel();
    let cargo_runner = CargoTestRunner::new_with_output(runner_config.clone(), output_tx)
        .with_cargo_config(cargo_config);

    let all_executables = cargo_runner.build_executables()?;

    if all_executables.is_empty() {
        pb.finish_with_message("No tests found.");
        return Ok(());
    }

    let target_executables = filter_for_files(&all_executables, &filtered_files);
    if target_executables.is_empty() {
        pb.finish_with_message("No tests found for the given target.");
        return Ok(());
    }

    let start_time = Instant::now();

    let result = if args.stream {
        let executables_clone = target_executables.clone();
        let handle = std::thread::spawn(move || cargo_runner.execute_tests(&executables_clone));

        let mut completed_tests = 0;
        let mut done_messages_received = 0;

        while completed_tests < target_executables.len() {
            pb.tick();
            match output_rx.try_recv() {
                Ok(output) => match output {
                    ProcessOutput::TestStarted { name, suite: _ } => {
                        pb.set_message(format!("{name}"));
                    }
                    ProcessOutput::TestPassed {
                        result,
                        duration_ms,
                    } => {
                        pb.inc(1);

                        let _ = pb.println(format!(
                            "{} {} {}",
                            "✓".bright_green().bold(),
                            result.name.bright_green(),
                            format!("({:.2}ms)", duration_ms).dimmed()
                        ));
                    }
                    ProcessOutput::TestFailed {
                        result,
                        duration_ms,
                        error,
                    } => {
                        pb.inc(1);

                        let _ = pb.println(format!(
                            "{} {} {}",
                            "✗".red().bold(),
                            result.name.red(),
                            format!("({:.2}ms)", duration_ms).dimmed()
                        ));

                        if !error.is_empty() && error != "Test failed" {
                            for error_line in error.lines() {
                                let _ = pb.println(format!("    {}", error_line.dimmed()));
                            }
                        } else if let Some(ref test_error) = result.error {
                            let _ = pb.println(format!("    {}", test_error.to_string().dimmed()));
                        }
                    }
                    ProcessOutput::TestSkipped { result } => {
                        pb.inc(1);

                        let _ =
                            pb.println(format!("{} {}", "○".yellow().bold(), result.name.yellow()));
                    }
                    ProcessOutput::SuiteStarted { name, test_count } => {
                        pb.set_message(format!("Starting {} ({} tests)", name, test_count));
                    }
                    ProcessOutput::SuiteCompleted { name: _ } => {
                        done_messages_received += 1;
                        if done_messages_received % 2 == 0 {
                            completed_tests += 1;
                            pb.set_prefix(format!("[{completed_tests}/{total_tests}]"));
                        }
                    }
                    ProcessOutput::Done => {
                        done_messages_received += 1;
                        if done_messages_received % 2 == 0 {
                            completed_tests += 1;
                            pb.set_prefix(format!("[{completed_tests}/{total_tests}]"));
                        }
                    }
                    _ => {}
                },
                Err(TryRecvError::Disconnected) => {
                    break;
                }
                Err(TryRecvError::Empty) => {
                    continue;
                }
            }
        }

        let mut result = handle.join().expect("Failed to complete test execution")?;

        result.finish(None);
        result
    } else {
        cargo_runner.execute_tests(&target_executables)?
    };

    let duration = start_time.elapsed();
    pb.finish_and_clear();

    display_test_results(&result, &args, duration)?;

    if args.output.is_some() {
        generate_report(&result, &args)?;
    }

    if !result.all_passed() {
        std::process::exit(1);
    }

    Ok(())
}

fn filter_for_files(
    executables: &[TestExecutable],
    test_files: &[TestFile],
) -> Vec<TestExecutable> {
    if test_files.is_empty() {
        return executables.to_vec();
    }

    let mut target_executables = Vec::new();

    for test_file in test_files {
        let file_path = test_file.path.to_string_lossy();
        let target_crate = determine_target_crate(&file_path);

        for executable in executables {
            let exec_name = &executable.name;
            let exec_path = executable.path.to_string_lossy();

            if exec_name.contains(&target_crate)
                || exec_path.contains(&target_crate)
                || executable.target_crate == target_crate
                || (target_crate == "examples"
                    && (exec_name.contains("sheila_examples")
                        || exec_path.contains("sheila_examples")))
            {
                if !target_executables
                    .iter()
                    .any(|e: &TestExecutable| e.path == executable.path)
                {
                    target_executables.push(executable.clone());
                }
            }
        }
    }

    if target_executables.is_empty() {
        executables.to_vec()
    } else {
        target_executables
    }
}

pub fn progress_wrapper(
    progress_bar: &ProgressBar,
    f: HashMap<String, impl FnOnce() -> sheila::Result<SuiteResult>>,
) -> color_eyre::Result<Vec<(String, SuiteResult)>> {
    let mut results = Vec::with_capacity(f.len());

    for (name, fnc) in f {
        progress_bar.set_message(format!("Running {}...", name));
        let result = fnc()?;
        results.push((name, result));
        progress_bar.set_position(results.len() as u64);
    }

    Ok(results)
}

fn display_test_results(
    result: &RunResult,
    args: &TestArgs,
    duration: Duration,
) -> color_eyre::Result<()> {
    let passed = result.passed_tests;
    let failed = result.failed_tests;
    let total = result.total_tests;

    println!(
        "{}",
        OutputFormatter::format_header(
            "Test Results",
            OutputFormatter::result_gradient(passed, total)
        )
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

fn generate_report(result: &RunResult, args: &TestArgs) -> color_eyre::Result<()> {
    let output_dir = args.output_dir.clone().unwrap_or_else(|| {
        Utils::get_default_output_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    });

    Utils::ensure_dir_exists(&output_dir)
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
