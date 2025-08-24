use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// Context object for tracking detailed information about
/// the test execution state.
#[derive(Debug, Clone)]
pub struct DebugContext {
    /// Breadcrumbs allow for tracking the current path of execution
    /// over the course of an individual test.
    ///
    /// These are generally formatted as:
    ///
    /// `test_suite_name::test_name::<point_in_call_stack>::<...children>`
    pub breadcrumbs: Vec<String>,
    /// Arbitrary debug data that can be added to the context at
    /// various points throughout the test execution lifecycle.
    pub data: HashMap<String, serde_json::Value>,

    pub config: DebugConfig,
}

impl DebugContext {
    pub fn new(config: DebugConfig) -> Self {
        Self {
            breadcrumbs: Vec::new(),
            data: HashMap::new(),
            config,
        }
    }

    pub fn add_breadcrumb<S: Into<String>>(&mut self, crumb: S) {
        self.breadcrumbs.push(crumb.into());
        if self.config.level >= DebugLevel::Debug {
            debug!("Breadcrumb: {}", self.breadcrumbs.last().unwrap());
        }
    }

    pub fn add_data<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: Into<String>,
        V: Serialize,
    {
        let json_value = serde_json::to_value(value)?;
        self.data.insert(key.into(), json_value);
        Ok(())
    }

    pub fn get_data<T>(&self, key: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.data
            .get(key)
            .ok_or_else(|| Error::generic(format!("Debug data '{}' not found", key)))
            .and_then(|v| serde_json::from_value(v.clone()).map_err(Error::from))
    }

    pub fn info<S: AsRef<str>>(&self, message: S) {
        if self.config.level >= DebugLevel::Info {
            info!("{}", message.as_ref());
        }
    }

    pub fn debug<S: AsRef<str>>(&self, message: S) {
        if self.config.level >= DebugLevel::Debug {
            debug!("{}", message.as_ref());
        }
    }

    pub fn warn<S: AsRef<str>>(&self, message: S) {
        if self.config.level >= DebugLevel::Info {
            warn!("{}", message.as_ref());
        }
    }

    pub fn error<S: AsRef<str>>(&self, message: S) {
        if self.config.level >= DebugLevel::Info {
            error!("{}", message.as_ref());
        }
    }

    pub fn current_path(&self) -> String {
        self.breadcrumbs.join(" -> ")
    }

    pub fn snapshot(&self) -> DebugSnapshot {
        DebugSnapshot {
            breadcrumbs: self.breadcrumbs.clone(),
            data: self.data.clone(),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Capture the state of a test run at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugSnapshot {
    pub breadcrumbs: Vec<String>,
    pub data: HashMap<String, serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub fn init_tracing(level: DebugLevel) -> Result<()> {
    let tracing_level = match level {
        DebugLevel::None => return Ok(()),
        DebugLevel::Info => tracing::Level::INFO,
        DebugLevel::Debug => tracing::Level::DEBUG,
        DebugLevel::Trace => tracing::Level::TRACE,
    };

    tracing_subscriber::fmt()
        .with_max_level(tracing_level)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .try_init()
        .map_err(|e| Error::generic(format!("Failed to initialize tracing: {}", e)))?;

    Ok(())
}

pub struct DebugFormatter;

impl DebugFormatter {
    pub fn format_duration(duration: &std::time::Duration) -> String {
        let millis = duration.as_millis();
        if millis < 1000 {
            format!("{}ms", millis)
        } else if millis < 60_000 {
            format!("{:.2}s", duration.as_secs_f64())
        } else {
            let minutes = duration.as_secs() / 60;
            let seconds = duration.as_secs() % 60;
            format!("{}m {}s", minutes, seconds)
        }
    }

    pub fn debug_value<T: std::fmt::Debug>(value: &T) -> String {
        format!("{:#?}", value)
    }

    pub fn pretty_json<T: Serialize>(value: &T) -> Result<String> {
        serde_json::to_string_pretty(value).map_err(Error::from)
    }

    pub fn truncate_string(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len.saturating_sub(3)])
        }
    }

    pub fn memory_info() -> HashMap<String, u64> {
        let mut info = HashMap::new();

        // This is a basic implementation - in a real scenario you might use
        // more sophisticated memory tracking
        #[cfg(unix)]
        {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<u64>() {
                                info.insert("rss_kb".to_string(), kb);
                            }
                        }
                    } else if line.starts_with("VmSize:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<u64>() {
                                info.insert("virtual_kb".to_string(), kb);
                            }
                        }
                    }
                }
            }
        }

        info
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DebugLevel {
    None,
    Info,
    Debug,
    Trace,
}

impl Default for DebugLevel {
    fn default() -> Self {
        DebugLevel::Info
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    pub level: DebugLevel,
    pub capture_output: bool,
    pub show_timing: bool,
    pub show_stack_traces: bool,
    pub custom: HashMap<String, serde_json::Value>,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            level: DebugLevel::default(),
            capture_output: true,
            show_timing: true,
            show_stack_traces: true,
            custom: HashMap::new(),
        }
    }
}
