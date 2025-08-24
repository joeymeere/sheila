use crate::cli::TestArgs;
use crate::config::SheilaConfig;
use crate::discovery::{TestDiscovery, TestFile};
use crate::helpers::{OutputFormatter, TargetSpec, parse_target};
use crate::helpers::{display_test_results, generate_report};
use colored::*;
use indicatif::ProgressBar;
use sheila::ProcessOutput;
use sheila::runners::{CargoRunnerConfig, CargoTestRunner, format_err_context};
use sheila::schemas::ExecutableBuilder;
use sheila::{Error, RunnerConfig};
use std::path::Path;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Instant;

pub fn run(args: TestArgs) -> color_eyre::Result<()> {
    println!();

    if args.headless {
        println!(
            "{}",
            OutputFormatter::format_warning("Headless mode not implemented")
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

    run_tests(args, filtered_files, &run_tests_pb, total_tests)?;

    Ok(())
}

fn run_discovery(
    args: TestArgs,
    pb: &ProgressBar,
) -> color_eyre::Result<(TestArgs, Vec<TestFile>, usize)> {
    let _config = SheilaConfig::load().map_err(|_| Error::generic("Failed to load config"))?;
    let discovery = TestDiscovery::new()?;
    let test_files = if let Some(target) = &args.target {
        let target_spec = parse_target(target);
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
    total_tests: usize,
) -> color_eyre::Result<()> {
    let mut runner_config = RunnerConfig::default();
    runner_config.fail_fast = args.fail_fast;

    if let Some(ref grep) = args.grep {
        runner_config.include_patterns.push(grep.clone());
    }

    runner_config.include_tags = args.tags.clone();

    let (output_tx, output_rx) = mpsc::channel();
    let mut cargo_config = CargoRunnerConfig::default();

    if let Some(timeout) = args.timeout {
        cargo_config
            .test_args
            .push(format!("--timeout={}", timeout));
    }

    let mut cargo_runner = CargoTestRunner::new_with_output(runner_config.clone(), output_tx)
        .with_cargo_config(cargo_config);

    let builder = ExecutableBuilder::new(None, None, vec![]);

    let target_executables = builder.exec()?;
    if target_executables.is_empty() {
        pb.finish_with_message("No tests found.");
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
                        location,
                    } => {
                        pb.inc(1);

                        let _ = pb.println(format!(
                            "{} {} {}",
                            "✗".red().bold(),
                            result.name.red(),
                            format!("({:.2}ms)", duration_ms).dimmed()
                        ));

                        if !error.is_empty() && location.is_some() {
                            let _ = pb.println(format_err_context(
                                &result.name,
                                location.clone(),
                                Some(&error),
                            ));
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
