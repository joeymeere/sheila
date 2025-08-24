use anyhow::Result;
use colored::Color;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct Utils;

impl Utils {
    pub fn get_most_recent_report(dir: &Path) -> Result<Option<PathBuf>> {
        if !dir.exists() {
            return Ok(None);
        }

        let mut most_recent: Option<(PathBuf, SystemTime)> = None;

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if matches!(ext.to_str(), Some("json") | Some("csv") | Some("html")) {
                        let metadata = fs::metadata(&path)?;
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
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn get_default_output_dir() -> Result<PathBuf> {
        let current = std::env::current_dir()?;
        Ok(current.join("test-results"))
    }

    pub fn validate_test_id(id: &str) -> color_eyre::Result<uuid::Uuid> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|_| sheila::Error::generic("Invalid test ID format. Expected a UUID."))?;
        Ok(uuid)
    }

    pub fn format_duration(duration: std::time::Duration) -> String {
        let total_seconds = duration.as_secs();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        let millis = duration.subsec_millis();

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else if seconds > 0 {
            format!("{}.{}s", seconds, millis / 100)
        } else {
            format!("{}ms", millis)
        }
    }

    pub fn format_file_size(size: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = size as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", size as u64, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }

    pub fn tag_color(tag: String) -> Color {
        let mut hasher = DefaultHasher::new();
        hasher.write(tag.as_bytes());
        let hash = hasher.finish();

        let colors = [
            Color::Red,
            Color::Green,
            Color::Yellow,
            Color::Blue,
            Color::Magenta,
            Color::Cyan,
            Color::White,
            Color::BrightRed,
            Color::BrightGreen,
            Color::BrightYellow,
            Color::BrightBlue,
            Color::BrightMagenta,
            Color::BrightCyan,
            Color::BrightWhite,
        ];

        colors[hash as usize % colors.len()]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TargetSpec {
    File(String),
    FileLine { file: String, line: usize },
    Function(String),
    Tag(String),
}
