use crate::output::OutputFormatter;
use crate::process::ProcessManager;
use crate::utils::Utils;
use anyhow::Result;
use std::fs;

pub async fn clear() -> color_eyre::Result<()> {
    println!("{}", OutputFormatter::format_info("Clearing all caches..."));

    let mut cleared_items: Vec<String> = Vec::new();
    let mut errors = Vec::new();

    match ProcessManager::new() {
        Ok(process_manager) => match process_manager.clear_cache().await {
            Ok(()) => cleared_items.push("Process cache".to_string()),
            Err(e) => errors.push(format!("Process cache: {}", e)),
        },
        Err(e) => errors.push(format!("Failed to initialize process manager: {}", e)),
    }

    match Utils::get_default_output_dir() {
        Ok(output_dir) => {
            if output_dir.exists() {
                match clear_directory(&output_dir) {
                    Ok(count) => {
                        if count > 0 {
                            let message = format!("Test results ({} files)", count);
                            cleared_items.push(message);
                        }
                    }
                    Err(e) => errors.push(format!("Test results cache: {}", e)),
                }
            }
        }
        Err(e) => errors.push(format!("Failed to get output directory: {}", e)),
    }

    match clear_temp_files().await {
        Ok(count) => {
            if count > 0 {
                let message = format!("Temporary files ({} files)", count);
                cleared_items.push(message);
            }
        }
        Err(e) => errors.push(format!("Temporary files: {}", e)),
    }

    match clear_compilation_cache().await {
        Ok(count) => {
            if count > 0 {
                let message = format!("Compilation cache ({} files)", count);
                cleared_items.push(message);
            }
        }
        Err(e) => errors.push(format!("Compilation cache: {}", e)),
    }

    if !cleared_items.is_empty() {
        println!(
            "{}",
            OutputFormatter::format_success("Successfully cleared:")
        );
        for item in &cleared_items {
            println!("  ✓ {}", item);
        }
    }

    if !errors.is_empty() {
        println!(
            "{}",
            OutputFormatter::format_warning("Some items could not be cleared:")
        );
        for error in &errors {
            println!("  ⚠ {}", error);
        }
    }

    if cleared_items.is_empty() && errors.is_empty() {
        println!(
            "{}",
            OutputFormatter::format_info("No cache files found to clear")
        );
    }

    println!(
        "{}",
        OutputFormatter::format_success("Cache clearing completed")
    );
    Ok(())
}

fn clear_directory(dir: &std::path::Path) -> Result<usize> {
    let mut count = 0;

    if !dir.exists() {
        return Ok(0);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            fs::remove_file(&path)?;
            count += 1;
        } else if path.is_dir() {
            let subdir_count = clear_directory(&path)?;
            count += subdir_count;

            if fs::read_dir(&path)?.next().is_none() {
                fs::remove_dir(&path)?;
            }
        }
    }

    Ok(count)
}

async fn clear_temp_files() -> Result<usize> {
    let mut count = 0;
    if let Some(temp_dir) = std::env::temp_dir().to_str() {
        let temp_path = std::path::Path::new(temp_dir);

        if temp_path.exists() {
            for entry in fs::read_dir(temp_path)? {
                let entry = entry?;
                let file_name = entry.file_name();

                if let Some(name) = file_name.to_str() {
                    if name.starts_with("sheila_") || name.starts_with(".sheila") {
                        let path = entry.path();
                        if path.is_file() {
                            fs::remove_file(&path)?;
                            count += 1;
                        } else if path.is_dir() {
                            let subdir_count = clear_directory(&path)?;
                            count += subdir_count;
                            fs::remove_dir_all(&path)?;
                        }
                    }
                }
            }
        }
    }

    if let Some(home_dir) = dirs::home_dir() {
        let sheila_temp = home_dir.join(".sheila").join("temp");
        if sheila_temp.exists() {
            let temp_count = clear_directory(&sheila_temp)?;
            count += temp_count;
        }
    }

    Ok(count)
}

async fn clear_compilation_cache() -> Result<usize> {
    let mut count = 0;

    let current_dir = std::env::current_dir()?;
    let target_dir = current_dir.join("target");

    if target_dir.exists() {
        let test_dirs = [
            target_dir.join("debug").join("deps"),
            target_dir.join("release").join("deps"),
        ];

        for test_dir in &test_dirs {
            if test_dir.exists() {
                for entry in fs::read_dir(test_dir)? {
                    let entry = entry?;
                    let file_name = entry.file_name();

                    if let Some(name) = file_name.to_str() {
                        if name.contains("test") || name.starts_with("sheila") {
                            let path = entry.path();
                            if path.is_file() {
                                fs::remove_file(&path)?;
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(count)
}
