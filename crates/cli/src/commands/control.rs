use crate::cli::ControlArgs;
use crate::output::OutputFormatter;
use crate::process::ProcessManager;
use crate::utils::Utils;
use colored::*;

pub async fn stop(args: ControlArgs) -> color_eyre::Result<()> {
    let test_id = Utils::validate_test_id(&args.test_id)?;
    let process_manager = ProcessManager::new()?;

    process_manager.load_from_cache().await?;

    if let Some(process) = process_manager.get_process(test_id).await {
        println!(
            "{}",
            OutputFormatter::format_info(&format!(
                "Stopping test process: {} (started: {})",
                test_id,
                process.started_at.format("%Y-%m-%d %H:%M:%S UTC")
            ))
        );

        process_manager.stop_process(test_id).await?;

        println!(
            "{}",
            OutputFormatter::format_success("Test process stopped successfully")
        );
    } else {
        println!(
            "{}",
            OutputFormatter::format_warning(&format!(
                "No running test process found with ID: {}",
                test_id
            ))
        );

        let processes = process_manager.list_processes().await;
        if !processes.is_empty() {
            println!("\nAvailable processes:");
            for process in processes {
                println!(
                    "  {} - {} ({})",
                    process.id,
                    process.command,
                    match process.status {
                        crate::process::ProcessStatus::Running => "running".green(),
                        crate::process::ProcessStatus::Paused => "paused".yellow(),
                        crate::process::ProcessStatus::Completed { .. } => "completed".blue(),
                        crate::process::ProcessStatus::Failed { .. } => "failed".red(),
                        crate::process::ProcessStatus::Stopped => "stopped".dimmed(),
                    }
                );
            }
        }
    }

    Ok(())
}

pub async fn pause(args: ControlArgs) -> color_eyre::Result<()> {
    let test_id = Utils::validate_test_id(&args.test_id)?;
    let process_manager = ProcessManager::new()?;

    process_manager.load_from_cache().await?;

    if let Some(process) = process_manager.get_process(test_id).await {
        match process.status {
            crate::process::ProcessStatus::Running => {
                println!(
                    "{}",
                    OutputFormatter::format_info(&format!("Pausing test process: {}", test_id))
                );

                process_manager.pause_process(test_id).await?;

                println!(
                    "{}",
                    OutputFormatter::format_success("Test process paused successfully")
                );
            }
            crate::process::ProcessStatus::Paused => {
                println!(
                    "{}",
                    OutputFormatter::format_warning(&format!(
                        "Test process {} is already paused",
                        test_id
                    ))
                );
            }
            _ => {
                println!(
                    "{}",
                    OutputFormatter::format_warning(&format!(
                        "Test process {} is not in a state that can be paused (status: {:?})",
                        test_id, process.status
                    ))
                );
            }
        }
    } else {
        println!(
            "{}",
            OutputFormatter::format_error(&format!("No test process found with ID: {}", test_id))
        );
    }

    Ok(())
}

pub async fn resume(args: ControlArgs) -> color_eyre::Result<()> {
    let test_id = Utils::validate_test_id(&args.test_id)?;
    let process_manager = ProcessManager::new()?;

    process_manager.load_from_cache().await?;

    if let Some(process) = process_manager.get_process(test_id).await {
        match process.status {
            crate::process::ProcessStatus::Paused => {
                println!(
                    "{}",
                    OutputFormatter::format_info(&format!("Resuming test process: {}", test_id))
                );

                process_manager.resume_process(test_id).await?;

                println!(
                    "{}",
                    OutputFormatter::format_success("Test process resumed successfully")
                );
            }
            crate::process::ProcessStatus::Running => {
                println!(
                    "{}",
                    OutputFormatter::format_warning(&format!(
                        "Test process {} is already running",
                        test_id
                    ))
                );
            }
            _ => {
                println!(
                    "{}",
                    OutputFormatter::format_warning(&format!(
                        "Test process {} is not in a state that can be resumed (status: {:?})",
                        test_id, process.status
                    ))
                );
            }
        }
    } else {
        println!(
            "{}",
            OutputFormatter::format_error(&format!("No test process found with ID: {}", test_id))
        );
    }

    Ok(())
}
