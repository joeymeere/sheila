use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFile {
    pub path: PathBuf,
    pub suites: Vec<TestSuite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuite {
    pub name: String,
    pub tests: Vec<TestFunction>,
    pub tags: Vec<String>,
    pub line_number: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFunction {
    pub name: String,
    pub tags: Vec<String>,
    pub line_number: Option<usize>,
    pub ignored: bool,
    pub timeout: Option<u64>,
    pub retries: Option<u32>,
}

pub struct TestDiscovery {
    _rust_file_pattern: Regex,
    test_function_pattern: Regex,
    suite_pattern: Regex,
}

impl TestDiscovery {
    pub fn new() -> color_eyre::Result<Self> {
        Ok(Self {
            _rust_file_pattern: Regex::new(r"\.rs$")?,
            test_function_pattern: Regex::new(
                r#"#\[sheila::test(?:\([^\)]*\))?\]\s*\n\s*(?:pub\s+)?fn\s+(\w+)"#,
            )?,
            suite_pattern: Regex::new(
                r#"#\[sheila::suite(?:\([^\)]*\))?\]\s*\n\s*(?:pub\s+)?struct\s+(\w+)"#,
            )?,
        })
    }

    pub fn discover(&self, path: &Path) -> color_eyre::Result<Vec<TestFile>> {
        if path.is_file() {
            if self.is_rust_file(path) {
                let test_file = self.parse_test_file(path)?;
                Ok(vec![test_file])
            } else {
                Ok(vec![])
            }
        } else if path.is_dir() {
            self.discover_in_directory(path)
        } else {
            Ok(vec![])
        }
    }

    pub fn discover_current(&self) -> color_eyre::Result<Vec<TestFile>> {
        let current_dir = std::env::current_dir()?;
        self.discover_in_directory(&current_dir)
    }

    fn is_rust_file(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "rs")
            .unwrap_or(false)
    }

    fn discover_in_directory(&self, dir: &Path) -> color_eyre::Result<Vec<TestFile>> {
        let mut test_files = Vec::new();

        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if self.is_rust_file(path) {
                if let Ok(test_file) = self.parse_test_file(path) {
                    if !test_file.suites.is_empty() {
                        test_files.push(test_file);
                    }
                }
            }
        }

        Ok(test_files)
    }

    fn parse_test_file(&self, path: &Path) -> color_eyre::Result<TestFile> {
        let content = fs::read_to_string(path)?;

        let suites = self.parse_suites(&content)?;

        Ok(TestFile {
            path: path.to_path_buf(),
            suites,
        })
    }

    fn parse_suites(&self, content: &str) -> color_eyre::Result<Vec<TestSuite>> {
        let mut suites = Vec::new();

        for suite_match in self.suite_pattern.captures_iter(content) {
            let suite_name = suite_match.get(1).unwrap().as_str().to_string();
            let match_start = suite_match.get(0).unwrap().start();
            let line_number = content[..match_start].lines().count();

            let suite = TestSuite {
                name: suite_name,
                tests: Vec::new(),
                tags: Vec::new(),
                line_number: Some(line_number),
            };

            suites.push(suite);
        }

        let standalone_tests = self.parse_test_functions(content)?;
        if !standalone_tests.is_empty() {
            suites.push(TestSuite {
                name: "Standalone Tests".to_string(),
                tests: standalone_tests,
                tags: Vec::new(),
                line_number: None,
            });
        }

        Ok(suites)
    }

    fn parse_test_functions(&self, content: &str) -> color_eyre::Result<Vec<TestFunction>> {
        let mut tests = Vec::new();

        for test_match in self.test_function_pattern.captures_iter(content) {
            let test_name = test_match.get(1).unwrap().as_str().to_string();
            let match_start = test_match.get(0).unwrap().start();
            let line_number = content[..match_start].lines().count();

            let attributes = self.parse_test_attributes(&test_match.get(0).unwrap().as_str());

            let test = TestFunction {
                name: test_name,
                tags: attributes
                    .get("tags")
                    .map(|tags| tags.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_default(),
                line_number: Some(line_number),
                ignored: attributes.contains_key("ignore"),
                timeout: attributes.get("timeout").and_then(|t| t.parse().ok()),
                retries: attributes.get("retries").and_then(|r| r.parse().ok()),
            };

            tests.push(test);
        }

        Ok(tests)
    }

    fn parse_test_attributes(&self, macro_text: &str) -> HashMap<String, String> {
        let mut attributes = HashMap::new();
        if let Some(attrs_start) = macro_text.find('(') {
            if let Some(attrs_end) = macro_text.rfind(')') {
                let attrs_text = &macro_text[attrs_start + 1..attrs_end];

                for attr in attrs_text.split(',') {
                    let attr = attr.trim();
                    if let Some(eq_pos) = attr.find('=') {
                        let key = attr[..eq_pos].trim();
                        let value = attr[eq_pos + 1..].trim().trim_matches('"');
                        attributes.insert(key.to_string(), value.to_string());
                    } else {
                        attributes.insert(attr.to_string(), "true".to_string());
                    }
                }
            }
        }

        attributes
    }

    pub fn filter_tests(
        &self,
        test_files: Vec<TestFile>,
        target: Option<&str>,
        tags: &[String],
        grep: Option<&str>,
    ) -> color_eyre::Result<Vec<TestFile>> {
        let grep_regex = if let Some(pattern) = grep {
            Some(Regex::new(pattern)?)
        } else {
            None
        };

        let mut filtered_files = Vec::new();

        for mut test_file in test_files {
            if let Some(target) = target {
                test_file = self.filter_by_target(test_file, target)?;
            }

            if !tags.is_empty() {
                test_file = self.filter_by_tags(test_file, tags);
            }

            if let Some(regex) = &grep_regex {
                test_file = self.filter_by_grep(test_file, regex);
            }

            if test_file.suites.iter().any(|suite| !suite.tests.is_empty()) {
                filtered_files.push(test_file);
            }
        }

        Ok(filtered_files)
    }

    fn filter_by_target(
        &self,
        mut test_file: TestFile,
        target: &str,
    ) -> color_eyre::Result<TestFile> {
        if target.contains(':') {
            let parts: Vec<&str> = target.split(':').collect();
            if parts.len() == 2 {
                let file_part = parts[0];
                if let Ok(line_num) = parts[1].parse::<usize>() {
                    if test_file.path.to_string_lossy().contains(file_part)
                        || test_file.path.ends_with(file_part)
                    {
                        for suite in &mut test_file.suites {
                            suite.tests.retain(|test| {
                                test.line_number
                                    .map(|ln| (ln as i32 - line_num as i32).abs() <= 5)
                                    .unwrap_or(false)
                            });
                        }
                    } else {
                        for suite in &mut test_file.suites {
                            suite.tests.clear();
                        }
                    }
                }
            }
        } else if target.ends_with(".rs") || target.contains('/') {
            if test_file.path.to_string_lossy().contains(target) || test_file.path.ends_with(target)
            {
            } else {
                for suite in &mut test_file.suites {
                    suite.tests.clear();
                }
            }
        } else {
            for suite in &mut test_file.suites {
                suite.tests.retain(|test| {
                    test.name.contains(target) || test.tags.iter().any(|tag| tag.contains(target))
                });
            }
        }

        Ok(test_file)
    }

    fn filter_by_tags(&self, mut test_file: TestFile, tags: &[String]) -> TestFile {
        for suite in &mut test_file.suites {
            suite
                .tests
                .retain(|test| tags.iter().any(|tag| test.tags.contains(tag)));
        }
        test_file
    }

    fn filter_by_grep(&self, mut test_file: TestFile, regex: &Regex) -> TestFile {
        for suite in &mut test_file.suites {
            suite.tests.retain(|test| {
                regex.is_match(&test.name) || test.tags.iter().any(|tag| regex.is_match(tag))
            });
        }
        test_file
    }
}
