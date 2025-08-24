use crate::test::TestContext;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Classes of hooks that can be registered, pertaining to
/// different stages of the execution lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookType {
    /// Run before all tests in a suite
    BeforeAll,
    /// Run after all tests in a suite
    AfterAll,
    /// Run before each test
    BeforeEach,
    /// Run after each test
    AfterEach,
    /// Run before fixture setup
    BeforeSetup,
    /// Run after fixture teardown
    AfterTeardown,
}

impl fmt::Display for HookType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HookType::BeforeAll => write!(f, "before_all"),
            HookType::AfterAll => write!(f, "after_all"),
            HookType::BeforeEach => write!(f, "before_each"),
            HookType::AfterEach => write!(f, "after_each"),
            HookType::BeforeSetup => write!(f, "before_setup"),
            HookType::AfterTeardown => write!(f, "after_teardown"),
        }
    }
}

#[derive(Clone)]
pub struct HookFn {
    pub name: String,
    pub function: fn(TestContext) -> Result<()>,
}

impl HookFn {
    pub fn new<S: Into<String>>(name: S, function: fn(TestContext) -> Result<()>) -> Self {
        Self {
            name: name.into(),
            function,
        }
    }

    pub fn execute(&self, context: TestContext) -> Result<()> {
        (self.function)(context)
    }
}

impl fmt::Debug for HookFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HookFn")
            .field("name", &self.name)
            .field("function", &"<function>")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct Hook {
    pub hook_type: HookType,
    pub function: HookFn,
    pub name: String,
    /// Whether this hook is required to succeed
    pub required: bool,
}

impl Hook {
    pub fn new<S: Into<String>>(
        hook_type: HookType,
        name: S,
        function: fn(TestContext) -> Result<()>,
    ) -> Self {
        let name_str = name.into();
        Self {
            hook_type,
            function: HookFn::new(name_str.clone(), function),
            name: name_str,
            required: true,
        }
    }

    /// Define whether the hook is required to succeed.
    ///
    /// If a hook is required and fails, the test will be marked as failed.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn execute(&self, context: TestContext) -> Result<()> {
        self.function.execute(context)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Hooks {
    pub before_all: Vec<Hook>,
    pub after_all: Vec<Hook>,
    pub before_each: Vec<Hook>,
    pub after_each: Vec<Hook>,
    pub before_setup: Vec<Hook>,
    pub after_teardown: Vec<Hook>,
}

impl Hooks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn before_all<S: Into<String>>(
        mut self,
        name: S,
        function: fn(TestContext) -> Result<()>,
    ) -> Self {
        self.before_all
            .push(Hook::new(HookType::BeforeAll, name, function));
        self
    }

    pub fn after_all<S: Into<String>>(
        mut self,
        name: S,
        function: fn(TestContext) -> Result<()>,
    ) -> Self {
        self.after_all
            .push(Hook::new(HookType::AfterAll, name, function));
        self
    }

    pub fn before_each<S: Into<String>>(
        mut self,
        name: S,
        function: fn(TestContext) -> Result<()>,
    ) -> Self {
        self.before_each
            .push(Hook::new(HookType::BeforeEach, name, function));
        self
    }

    pub fn after_each<S: Into<String>>(
        mut self,
        name: S,
        function: fn(TestContext) -> Result<()>,
    ) -> Self {
        self.after_each
            .push(Hook::new(HookType::AfterEach, name, function));
        self
    }

    /// Iteratively execute all hooks of a given type
    pub fn execute(&self, hook_type: HookType, context: &TestContext) -> Result<()> {
        let hooks = match hook_type {
            HookType::BeforeAll => &self.before_all,
            HookType::AfterAll => &self.after_all,
            HookType::BeforeEach => &self.before_each,
            HookType::AfterEach => &self.after_each,
            HookType::BeforeSetup => &self.before_setup,
            HookType::AfterTeardown => &self.after_teardown,
        };

        for hook in hooks {
            hook.execute(context.clone()).map_err(|e| {
                Error::hook(
                    hook_type.to_string(),
                    format!("Hook '{}' failed: {}", hook.name, e),
                )
            })?;
        }

        Ok(())
    }

    /// Get all hooks of a given type
    pub fn get_hooks(&self, hook_type: HookType) -> &[Hook] {
        match hook_type {
            HookType::BeforeAll => &self.before_all,
            HookType::AfterAll => &self.after_all,
            HookType::BeforeEach => &self.before_each,
            HookType::AfterEach => &self.after_each,
            HookType::BeforeSetup => &self.before_setup,
            HookType::AfterTeardown => &self.after_teardown,
        }
    }

    /// Check if any hooks of a given type exist
    pub fn has_hooks(&self, hook_type: HookType) -> bool {
        !self.get_hooks(hook_type).is_empty()
    }

    pub fn total_hooks(&self) -> usize {
        self.before_all.len()
            + self.after_all.len()
            + self.before_each.len()
            + self.after_each.len()
            + self.before_setup.len()
            + self.after_teardown.len()
    }
}
