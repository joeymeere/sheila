use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::Result;

use crate::helpers::TargetSpec;

pub fn get_most_recent_report(dir: &Path) -> Result<Option<PathBuf>> {
    if !dir.exists() {
        return Ok(None);
    }

    let mut most_recent: Option<(PathBuf, SystemTime)> = None;

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                if matches!(ext.to_str(), Some("json") | Some("csv") | Some("html")) {
                    let metadata = std::fs::metadata(&path)?;
                    let modified = metadata.modified()?;

                    match most_recent {
                        Some((_, ref latest)) if modified > *latest => {
                            most_recent = Some((path, modified));
                        }
                        None => {
                            most_recent = Some((path, modified));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(most_recent.map(|(path, _)| path))
}

pub fn parse_target(target: &str) -> TargetSpec {
    if target.contains(':') {
        let parts: Vec<&str> = target.splitn(2, ':').collect();
        if parts.len() == 2 {
            if let Ok(line) = parts[1].parse::<usize>() {
                return TargetSpec::FileLine {
                    file: parts[0].to_string(),
                    line,
                };
            }
        }
    }

    if target.starts_with('@') {
        TargetSpec::Tag(target[1..].to_string())
    } else if target.contains('/') || target.ends_with(".rs") {
        TargetSpec::File(target.to_string())
    } else {
        TargetSpec::Function(target.to_string())
    }
}

pub fn ensure_dir_exists(dir: &Path) -> Result<()> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

pub fn get_default_output_dir() -> Result<PathBuf> {
    let current = std::env::current_dir()?;
    Ok(current.join("test-results"))
}
