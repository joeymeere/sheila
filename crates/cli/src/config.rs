use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheilaConfig {
    pub build: BuildConfig,
    pub discovery: DiscoveryConfig,
    pub reporting: ReportingConfig,
    pub runner: RunnerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub target_dir: PathBuf,
    pub debug_dir: String,
    pub release_dir: String,
    pub deps_dir: String,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub rust_file_extensions: Vec<String>,
    pub test_patterns: Vec<String>,
    pub suite_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportingConfig {
    pub output_dir: PathBuf,
    pub formats: Vec<String>,
    pub timestamp_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub default_timeout: u64,
    pub max_retries: u32,
    pub parallel_limit: Option<usize>,
}

impl Default for SheilaConfig {
    fn default() -> Self {
        Self {
            build: BuildConfig {
                target_dir: PathBuf::from("target"),
                debug_dir: "debug".to_string(),
                release_dir: "release".to_string(),
                deps_dir: "deps".to_string(),
                profile: "debug".to_string(),
            },
            discovery: DiscoveryConfig {
                rust_file_extensions: vec!["rs".to_string()],
                test_patterns: vec![
                    r#"#\[sheila::test(?:\([^\)]*\))?\]\s*\n\s*(?:pub\s+)?fn\s+(\w+)"#.to_string(),
                ],
                suite_patterns: vec![
                    r#"#\[sheila::suite(?:\([^\)]*\))?\]\s*\n\s*(?:pub\s+)?struct\s+(\w+)"#
                        .to_string(),
                ],
                exclude_patterns: vec!["target/**".to_string(), "**/.git/**".to_string()],
            },
            reporting: ReportingConfig {
                output_dir: PathBuf::from("test-results"),
                formats: vec!["json".to_string(), "html".to_string()],
                timestamp_format: "%Y%m%d_%H%M%S".to_string(),
            },
            runner: RunnerConfig {
                default_timeout: 30,
                max_retries: 3,
                parallel_limit: None,
            },
        }
    }
}

impl SheilaConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Try to load from sheila.toml, fall back to default
        if let Ok(content) = std::fs::read_to_string("sheila.toml") {
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn build_target_path(&self, profile: Option<&str>) -> PathBuf {
        let profile = profile.unwrap_or(&self.build.profile);
        self.build
            .target_dir
            .join(profile)
            .join(&self.build.deps_dir)
    }
}
