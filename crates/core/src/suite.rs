use crate::fixtures::FixtureRegistry;
use crate::internal::HookFn;
use crate::test::{TestContext, TestResult};
use crate::{Error, Result, Test, TestMetadata, TestStatus};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Deref;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug)]
pub struct TestSuite {
    pub id: Uuid,
    pub name: String,
    pub module_path: String,
    pub tests: IndexMap<String, Test>,
    pub attributes: SuiteAttributes,
    pub meta: TestMetadata,
    pub hooks: SuiteHooks,
    pub fixtures: FixtureRegistry,
    pub shared_data: IndexMap<String, serde_json::Value>,
}

impl TestSuite {
    pub fn new<S: Into<String>>(name: S) -> Self {
        let name = name.into();
        Self {
            id: Uuid::new_v4(),
            name: name.clone(),
            module_path: String::new(),
            tests: IndexMap::new(),
            attributes: SuiteAttributes::default(),
            meta: TestMetadata::new(name),
            hooks: SuiteHooks::new(),
            fixtures: FixtureRegistry::new(),
            shared_data: IndexMap::new(),
        }
    }

    pub fn new_with_module<S: Into<String>, M: Into<String>>(name: S, module_path: M) -> Self {
        let name = name.into();
        let module_path = module_path.into();
        Self {
            id: Uuid::new_v4(),
            name: name.clone(),
            module_path,
            tests: IndexMap::new(),
            attributes: SuiteAttributes::default(),
            meta: TestMetadata::new(name),
            hooks: SuiteHooks::new(),
            fixtures: FixtureRegistry::new(),
            shared_data: IndexMap::new(),
        }
    }

    pub fn new_with_hooks<S: Into<String>>(name: S, hooks: SuiteHooks) -> Self {
        let name = name.into();

        Self {
            id: Uuid::new_v4(),
            name: name.clone(),
            module_path: String::new(),
            tests: IndexMap::new(),
            attributes: SuiteAttributes::default(),
            meta: TestMetadata::new(name),
            hooks,
            fixtures: FixtureRegistry::new(),
            shared_data: IndexMap::new(),
        }
    }

    pub fn add_test(mut self, test: Test) -> Self {
        self.tests.insert(test.meta.name.clone(), test);
        self
    }

    pub fn with_attributes(mut self, attributes: SuiteAttributes) -> Self {
        self.attributes = attributes;
        self
    }

    pub fn with_metadata(mut self, meta: TestMetadata) -> Self {
        self.meta = meta;
        self
    }

    pub fn with_hooks(mut self, hooks: SuiteHooks) -> Self {
        self.hooks = hooks;
        self
    }

    pub fn with_fixtures(mut self, registry: FixtureRegistry) -> Self {
        self.fixtures = registry;
        self
    }

    pub fn module_path(&self) -> &str {
        &self.module_path
    }

    pub fn is_in_module(&self, module: &str) -> bool {
        self.module_path.starts_with(module)
    }

    pub fn get_tests(&self) -> impl Iterator<Item = &Test> {
        self.tests.values()
    }

    pub fn get_runnable_tests(&self) -> Vec<&Test> {
        if self.attributes.only {
            return self.tests.values().collect();
        }

        let only_tests: Vec<&Test> = self.tests.values().filter(|test| test.is_only()).collect();

        if !only_tests.is_empty() {
            return only_tests;
        }

        self.tests
            .values()
            .filter(|test| !test.should_ignore())
            .collect()
    }

    pub fn ignore(mut self) -> Self {
        self.attributes.ignore = true;
        self
    }

    pub fn only(mut self) -> Self {
        self.attributes.only = true;
        self
    }

    pub fn retries(mut self, count: u32) -> Self {
        self.attributes.retries = count;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.attributes.timeout = Some(timeout);
        self
    }

    pub fn tag<S: Into<String>>(mut self, tag: S) -> Self {
        self.attributes.tags.push(tag.into());
        self
    }

    pub fn category<S: Into<String>>(mut self, category: S) -> Self {
        self.attributes.category = Some(category.into());
        self
    }

    pub fn set_shared_data<T: Serialize>(mut self, key: String, value: T) -> Result<Self> {
        let json_value = serde_json::to_value(value)?;
        self.shared_data.insert(key, json_value);
        Ok(self)
    }

    pub fn should_ignore(&self) -> bool {
        self.attributes.ignore
    }

    pub fn is_only(&self) -> bool {
        self.attributes.only
    }

    pub fn get_timeout(&self) -> Option<Duration> {
        self.attributes.timeout
    }

    pub fn execute(&mut self) -> Result<SuiteResult> {
        let mut result = SuiteResult::new(self.id, self.name.clone(), self.meta.clone());
        let suite_context = TestContext::new(self.id, self.meta.clone());

        if let Err(e) = self.fixtures.setup_suite_fixtures(&suite_context) {
            result.finish(Some(e));
            return Ok(result);
        }

        if let Err(e) =
            self.hooks
                .execute_hooks(&self.hooks.before_all, &suite_context, "before_all")
        {
            result.finish(Some(e));
            return Ok(result);
        }

        let runnable_test_info: Vec<(Uuid, String, TestMetadata)> = self
            .get_runnable_tests()
            .iter()
            .map(|test| (test.id, test.meta.name.clone(), test.meta.clone()))
            .collect();

        for (test_id, test_name, test_meta) in runnable_test_info {
            let test_context = TestContext::new(test_id, test_meta.clone());

            if let Err(e) = self.fixtures.setup_test_fixtures(&test_context) {
                let mut test_result =
                    TestResult::new(test_id, test_name.clone(), test_meta.clone());
                test_result.finish(TestStatus::Failed, Some(e));
                result.add_test_result(test_result);
                continue;
            }

            if let Err(e) =
                self.hooks
                    .execute_hooks(&self.hooks.before_each, &test_context, "before_each")
            {
                let mut test_result =
                    TestResult::new(test_id, test_name.clone(), test_meta.clone());
                test_result.finish(TestStatus::Failed, Some(e));
                result.add_test_result(test_result);

                let _ = self.fixtures.teardown_test_fixtures(&test_context);
                continue;
            }

            let mut test_result = if let Some(test) = self.tests.get(&test_name) {
                test.execute(test_context.clone())
            } else {
                let mut result = TestResult::new(test_id, test_name.clone(), test_meta.clone());
                result.finish(TestStatus::Failed, Some(Error::generic("Test not found")));
                result
            };

            if let Err(e) =
                self.hooks
                    .execute_hooks(&self.hooks.after_each, &test_context, "after_each")
            {
                if test_result.passed() {
                    test_result.finish(TestStatus::Failed, Some(e));
                }
            }

            if let Err(e) = self.fixtures.teardown_test_fixtures(&test_context) {
                eprintln!("Warning: fixture teardown failed: {}", e);
            }

            result.add_test_result(test_result);
        }

        if let Err(e) = self
            .hooks
            .execute_hooks(&self.hooks.after_all, &suite_context, "after_all")
        {
            result.finish(Some(e));
            return Ok(result);
        }

        if let Err(e) = self.fixtures.teardown_suite_fixtures(&suite_context) {
            result.finish(Some(e));
            return Ok(result);
        }

        result.finish(None);
        Ok(result)
    }
}

impl Deref for TestSuite {
    type Target = IndexMap<String, Test>;

    fn deref(&self) -> &Self::Target {
        &self.tests
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteAttributes {
    pub ignore: bool,
    pub only: bool,
    pub retries: u32,
    pub timeout: Option<Duration>,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub parallel: bool,
    pub max_concurrent: Option<usize>,
    pub custom: HashMap<String, serde_json::Value>,
}

impl Default for SuiteAttributes {
    fn default() -> Self {
        Self {
            ignore: false,
            only: false,
            retries: 0,
            timeout: None,
            tags: Vec::new(),
            category: None,
            parallel: false,
            max_concurrent: None,
            custom: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteResult {
    pub id: Uuid,
    pub name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<Duration>,
    pub test_results: Vec<TestResult>,
    pub metadata: TestMetadata,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub skipped_tests: usize,
    pub error: Option<Error>,
}

impl SuiteResult {
    pub fn new(id: Uuid, name: String, metadata: TestMetadata) -> Self {
        Self {
            id,
            name,
            start_time: Utc::now(),
            end_time: None,
            duration: None,
            test_results: Vec::new(),
            metadata,
            total_tests: 0,
            passed_tests: 0,
            failed_tests: 0,
            skipped_tests: 0,
            error: None,
        }
    }

    pub fn add_test_result(&mut self, result: TestResult) {
        self.total_tests += 1;

        match result.status {
            TestStatus::Passed => self.passed_tests += 1,
            TestStatus::Failed | TestStatus::Timeout => self.failed_tests += 1,
            TestStatus::Skipped | TestStatus::Ignored => self.skipped_tests += 1,
            _ => {}
        }

        self.test_results.push(result);
    }

    pub fn finish(&mut self, error: Option<Error>) {
        self.end_time = Some(Utc::now());
        self.error = error;

        if let Some(end_time) = self.end_time {
            self.duration = Some(Duration::from_millis(
                (end_time - self.start_time).num_milliseconds() as u64,
            ));
        }
    }

    pub fn all_passed(&self) -> bool {
        self.failed_tests == 0 && self.error.is_none()
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_tests == 0 {
            return 1.0;
        }
        self.passed_tests as f64 / self.total_tests as f64
    }
}

#[derive(Debug, Default, Clone)]
pub struct SuiteHooks {
    pub before_all: Vec<HookFn>,
    pub after_all: Vec<HookFn>,
    pub before_each: Vec<HookFn>,
    pub after_each: Vec<HookFn>,
}

impl SuiteHooks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn before_all<S: Into<String>>(
        mut self,
        name: S,
        hook: fn(TestContext) -> Result<()>,
    ) -> Self {
        self.before_all.push(HookFn::new(name, hook));
        self
    }

    pub fn after_all<S: Into<String>>(
        mut self,
        name: S,
        hook: fn(TestContext) -> Result<()>,
    ) -> Self {
        self.after_all.push(HookFn::new(name, hook));
        self
    }

    pub fn before_each<S: Into<String>>(
        mut self,
        name: S,
        hook: fn(TestContext) -> Result<()>,
    ) -> Self {
        self.before_each.push(HookFn::new(name, hook));
        self
    }

    pub fn after_each<S: Into<String>>(
        mut self,
        name: S,
        hook: fn(TestContext) -> Result<()>,
    ) -> Self {
        self.after_each.push(HookFn::new(name, hook));
        self
    }

    pub fn execute_hooks(
        &self,
        hooks: &[HookFn],
        context: &TestContext,
        hook_type: &str,
    ) -> Result<()> {
        for hook in hooks {
            hook.execute(context.clone()).map_err(|e| {
                Error::hook(
                    hook_type.to_string(),
                    format!("Hook '{}' execution failed: {}", hook.name, e),
                )
            })?;
        }
        Ok(())
    }
}
