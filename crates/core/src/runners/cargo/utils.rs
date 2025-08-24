use colored::Colorize;

use crate::SourceLocation;

pub fn format_mod_name(name: &str) -> String {
    if name.contains("__sheila_") {
        let re = regex::Regex::new(r"::(__sheila_[^:]+_tests)::").unwrap();
        let name = re.replace(name, "::");
        name.to_string()
    } else {
        name.to_string()
    }
}

pub fn format_err_context(
    name: &str,
    location: Option<SourceLocation>,
    msg: Option<&str>,
) -> String {
    match (location, msg) {
        (Some(location), Some(message)) => {
            let arrow = "-->".bright_red();
            let border = "|".dimmed();
            let line_number = location.line.to_string().bright_red();
            let caret = "^^^".bright_red().bold();
            let message = message.bright_red().bold();
            let placeholder = "<source code placeholder>".dimmed();

            let path =
                format!("{}:{}:{}", location.file, location.line, location.column).bright_red();

            let column_spaces = " ".repeat((location.column as usize).saturating_sub(1));

            format!(
                "    {arrow} {path}\n    {border}\n{line_number} {border} {placeholder}\n    {border} {column_spaces}{caret} {message}\n    {border}",
            )
        }
        (Some(location), None) => {
            let arrow = "-->".bright_red();
            let path =
                format!("{}:{}:{}", location.file, location.line, location.column).bright_red();
            format!("    {arrow} {path}")
        }
        (None, Some(message)) => {
            format!("Test panicked: {}", message.bright_red())
        }
        (None, None) => {
            format!("Test '{}' failed - check test output for details", name)
        }
    }
}
