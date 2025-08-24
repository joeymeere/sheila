use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockCall {
    pub fn_name: String,
    pub args: Vec<serde_json::Value>,
    pub ts: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct MockConfig {
    pub expected_calls: Option<usize>,
    pub return_values: Vec<serde_json::Value>,
    pub panic_on_unexpected: bool,
    pub validator: Option<Arc<dyn Fn(&[serde_json::Value]) -> Result<()> + Send + Sync>>,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            expected_calls: None,
            return_values: Vec::new(),
            panic_on_unexpected: false,
            validator: None,
        }
    }
}

impl std::fmt::Debug for MockConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockConfig")
            .field("expected_calls", &self.expected_calls)
            .field("return_values", &self.return_values)
            .field("panic_on_unexpected", &self.panic_on_unexpected)
            .field("validator", &self.validator.as_ref().map(|_| "<function>"))
            .finish()
    }
}

#[derive(Default)]
pub struct MockCollection {
    configs: HashMap<String, MockConfig>,
    calls: Arc<Mutex<Vec<MockCall>>>,
    call_counts: Arc<Mutex<HashMap<String, usize>>>,
}

impl MockCollection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_mock<S: Into<String>>(&mut self, function_name: S, config: MockConfig) {
        self.configs.insert(function_name.into(), config);
    }

    pub fn record_call<S: Into<String>>(
        &self,
        function_name: S,
        arguments: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let function_name = function_name.into();

        let call = MockCall {
            fn_name: function_name.clone(),
            args: arguments.clone(),
            ts: chrono::Utc::now(),
        };

        {
            let mut calls = self.calls.lock().unwrap();
            calls.push(call);
        }

        {
            let mut counts = self.call_counts.lock().unwrap();
            *counts.entry(function_name.clone()).or_insert(0) += 1;
        }

        let config = self.configs.get(&function_name);

        if let Some(config) = config {
            if let Some(ref validator) = config.validator {
                validator(&arguments)?;
            }

            if let Some(expected) = config.expected_calls {
                let current_count = self.get_call_count(&function_name);
                if current_count > expected {
                    if config.panic_on_unexpected {
                        panic!(
                            "Unexpected call to '{}': expected {} calls, got {}",
                            function_name, expected, current_count
                        );
                    } else {
                        return Err(Error::mock(format!(
                            "Unexpected call to '{}': expected {} calls, got {}",
                            function_name, expected, current_count
                        )));
                    }
                }
            }

            let call_index = self.get_call_count(&function_name) - 1;
            if let Some(return_value) = config.return_values.get(call_index) {
                Ok(return_value.clone())
            } else if !config.return_values.is_empty() {
                Ok(config.return_values.last().unwrap().clone())
            } else {
                Ok(serde_json::Value::Null)
            }
        } else {
            Ok(serde_json::Value::Null)
        }
    }

    pub fn get_call_count(&self, fn_name: &str) -> usize {
        self.call_counts
            .lock()
            .unwrap()
            .get(fn_name)
            .copied()
            .unwrap_or(0)
    }

    pub fn get_calls(&self, fn_name: &str) -> Vec<MockCall> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call.fn_name == fn_name)
            .cloned()
            .collect()
    }

    pub fn get_all_calls(&self) -> Vec<MockCall> {
        self.calls.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.calls.lock().unwrap().clear();
        self.call_counts.lock().unwrap().clear();
    }

    pub fn verify(&self) -> Result<()> {
        for (function_name, config) in &self.configs {
            if let Some(expected_calls) = config.expected_calls {
                let actual_calls = self.get_call_count(function_name);
                if actual_calls != expected_calls {
                    return Err(Error::mock(format!(
                        "Mock verification failed for '{}': expected {} calls, got {}",
                        function_name, expected_calls, actual_calls
                    )));
                }
            }
        }
        Ok(())
    }
}

pub struct MockBuilder {
    config: MockConfig,
}

impl MockBuilder {
    pub fn new() -> Self {
        Self {
            config: MockConfig::default(),
        }
    }

    /// Define expected number of calls for a given mock
    ///
    /// If the number of calls is exceeded, the mock will panic.
    pub fn expect_calls(mut self, count: usize) -> Self {
        self.config.expected_calls = Some(count);
        self
    }

    /// Define a return value for a given mock
    ///
    /// If no return values are defined, the mock will return `null`.
    pub fn returns<T: Serialize>(mut self, value: T) -> Result<Self> {
        let json_value = serde_json::to_value(value)?;
        self.config.return_values.push(json_value);
        Ok(self)
    }

    /// Define multiple return values for a given mock
    ///
    /// If no return values are defined, the mock will return `null`.
    pub fn returns_sequence<T: Serialize>(mut self, values: Vec<T>) -> Result<Self> {
        for value in values {
            let json_value = serde_json::to_value(value)?;
            self.config.return_values.push(json_value);
        }
        Ok(self)
    }

    /// Define whether the mock should panic on unexpected calls
    ///
    /// If a mock is configured to panic on unexpected calls,
    /// the test will be marked as failed if the number of calls
    /// is exceeded.
    pub fn panic_on_unexpected(mut self, panic: bool) -> Self {
        self.config.panic_on_unexpected = panic;
        self
    }

    /// Define a custom validator for a given mock
    ///
    /// The validator is a function that takes the arguments of the
    /// mock call and returns a `Result`. If the validator returns
    /// an error, the mock will be marked as failed.
    pub fn with_validator<F>(mut self, validator: F) -> Self
    where
        F: Fn(&[serde_json::Value]) -> Result<()> + Send + Sync + 'static,
    {
        self.config.validator = Some(Arc::new(validator));
        self
    }

    pub fn build(self) -> MockConfig {
        self.config
    }
}

impl Default for MockBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "global-mocks")]
pub mod global {
    use super::*;

    static GLOBAL_MOCKS: std::sync::OnceLock<Arc<Mutex<MockCollection>>> =
        std::sync::OnceLock::new();

    pub fn global_mocks() -> Arc<Mutex<MockCollection>> {
        GLOBAL_MOCKS
            .get_or_init(|| Arc::new(Mutex::new(MockCollection::new())))
            .clone()
    }

    pub fn set_global_mock<S: Into<String>>(function_name: S, config: MockConfig) {
        let registry = global_mocks();
        let mut registry = registry.lock().unwrap();
        registry.register_mock(function_name, config);
    }

    pub fn record_mock_call_global<S: Into<String>>(
        function_name: S,
        arguments: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let registry = global_mocks();
        let registry = registry.lock().unwrap();
        registry.record_call(function_name, arguments)
    }

    pub fn call_count_global(function_name: &str) -> usize {
        let registry = global_mocks();
        let registry = registry.lock().unwrap();
        registry.get_call_count(function_name)
    }

    pub fn verify_mocks_global() -> Result<()> {
        let registry = global_mocks();
        let registry = registry.lock().unwrap();
        registry.verify()
    }

    pub fn clear_mocks_global() {
        let registry = global_mocks();
        let registry = registry.lock().unwrap();
        registry.clear();
    }
}
