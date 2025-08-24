pub mod files;
pub mod output;
pub mod report;

pub use files::*;
pub use output::*;
pub use report::*;

use colored::Color;

use std::hash::{DefaultHasher, Hasher};
use tiny_gradient::Gradient;

pub fn validate_test_id(id: &str) -> color_eyre::Result<uuid::Uuid> {
    let uuid = uuid::Uuid::parse_str(id)
        .map_err(|_| sheila::Error::generic("Invalid test ID format. Expected a UUID."))?;
    Ok(uuid)
}

pub fn result_gradient(passed: usize, total: usize) -> Gradient {
    if passed < total / 2 {
        Gradient::Instagram
    } else if passed < total / 4 * 3 {
        Gradient::Morning
    } else {
        Gradient::Vice
    }
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

pub enum TargetSpec {
    File(String),
    FileLine { file: String, line: usize },
    Function(String),
    Tag(String),
}
