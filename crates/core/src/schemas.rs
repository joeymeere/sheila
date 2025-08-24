use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::Stdio,
};

use cmdstruct::Command;

use crate::{Error, Result, TestExecutable};

#[derive(Command)]
#[command(executable = "cargo")]
pub struct ExecutableBuilder {
    #[arg]
    sub: String,

    #[arg(option = "--filter")]
    filter: Option<String>,

    #[arg(option = "--profile")]
    profile: Option<String>,

    #[arg(option = "--cargo")]
    cargo: Vec<String>,
}

impl ExecutableBuilder {
    pub fn new(filter: Option<String>, profile: Option<String>, cargo: Vec<String>) -> Self {
        Self {
            sub: "test".to_string(),
            filter,
            profile,
            cargo,
        }
    }

    pub fn args(&self) -> Result<Vec<String>> {
        let mut cargo_args = vec![self.sub.clone()];

        if let Some(filter) = &self.filter {
            cargo_args.push(format!("--filter={}", filter));
        }

        cargo_args.extend_from_slice(&[
            "--no-run".to_string(),
            "--tests".to_string(),
            "--verbose".to_string(),
            "--workspace".to_string(),
            "--message-format=json-diagnostic-rendered-ansi".to_string(),
        ]);

        if let Some(ref profile) = self.profile {
            cargo_args.extend_from_slice(&["--profile".to_string(), profile.clone()]);
        }

        cargo_args.extend_from_slice(&[
            "--features".to_string(),
            [
                "sheila-proc-macros/__sheila_test".to_string(),
                "sheila/full".to_string(),
            ]
            .join(","),
        ]);

        cargo_args.extend_from_slice(&self.cargo);
        Ok(cargo_args)
    }

    pub fn exec(&self) -> Result<Vec<TestExecutable>> {
        let args = self.args()?;
        let mut child = self
            .command()
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::test_execution(format!("Failed to spawn cargo build: {}", e)))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::test_execution("Failed to capture cargo build stdout"))?;

        let mut executables = Vec::new();
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            let line = line.map_err(|e| {
                Error::test_execution(format!("Failed to read cargo output: {}", e))
            })?;

            if let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(executable) = self.extract_test_executable(&message)? {
                    executables.push(executable);
                }
            }
        }

        let exit_status = child
            .wait()
            .map_err(|e| Error::test_execution(format!("Failed to wait for cargo build: {}", e)))?;

        if !exit_status.success() {
            return Err(Error::test_execution(format!(
                "Cargo build failed with exit code: {:?}",
                exit_status.code()
            )));
        }

        Ok(executables)
    }

    fn extract_test_executable(
        &self,
        message: &serde_json::Value,
    ) -> Result<Option<TestExecutable>> {
        let reason = message
            .get("reason")
            .and_then(|r| r.as_str())
            .ok_or_else(|| Error::test_execution("Missing reason field in cargo output"))?;

        if reason != "compiler-artifact" {
            return Ok(None);
        }

        let package_id = message
            .get("package_id")
            .and_then(|p| p.as_str())
            .ok_or_else(|| Error::test_execution("Missing package_id field"))?;

        let package_name = package_id
            .split_whitespace()
            .next()
            .ok_or_else(|| Error::test_execution("Invalid package_id format"))?
            .to_string();

        let profile = message
            .get("profile")
            .ok_or_else(|| Error::test_execution("Missing profile field"))?;

        let is_test = profile
            .get("test")
            .and_then(|t| t.as_bool())
            .ok_or_else(|| Error::test_execution("Missing test field in profile"))?;

        if !is_test {
            return Ok(None);
        }

        if let Some(executable_path) = message.get("executable").and_then(|e| e.as_str()) {
            let target = message
                .get("target")
                .ok_or_else(|| Error::test_execution("Missing target field"))?;

            let name = target
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| Error::test_execution("Missing name field in target"))?
                .to_string();

            Ok(Some(TestExecutable::new(
                PathBuf::from(executable_path),
                name,
                package_name,
            )))
        } else {
            Ok(None)
        }
    }

    pub fn filter_executables(
        &self,
        executables: &[TestExecutable],
        target_filter: Option<&str>,
    ) -> Vec<TestExecutable> {
        if let Some(target) = target_filter {
            let target_crate = if target.contains('/') || target.ends_with(".rs") {
                TestExecutable::determine_target_crate(&PathBuf::from(target))
            } else {
                target.to_string()
            };

            let filtered = executables
                .iter()
                .filter(|exe| {
                    let matches = exe.target_crate == target_crate
                        || exe.name.contains(&target_crate)
                        || exe.path.to_string_lossy().contains(&target_crate)
                        || (target_crate == "examples"
                            && (exe.name.contains("sheila_examples")
                                || exe.path.to_string_lossy().contains("sheila_examples")));

                    matches
                })
                .cloned()
                .collect::<Vec<_>>();

            filtered
        } else {
            executables.to_vec()
        }
    }
}
