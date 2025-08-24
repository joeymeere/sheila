use crate::Result;
use crate::test::TestContext;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use strum_macros::EnumString;
use uuid::Uuid;

/// Fixture scope determines when fixtures are created and destroyed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum FixtureScope {
    /// Created once per test run and shared across all tests
    #[strum(serialize = "session")]
    Session,
    /// Created once per test suite and shared across tests in that suite
    #[strum(serialize = "suite")]
    Suite,
    /// Created once per test
    #[strum(serialize = "test")]
    Test,
    /// Created for each test invocation (useful for parameterized tests)
    #[strum(serialize = "invocation")]
    Invocation,
}

impl Default for FixtureScope {
    fn default() -> Self {
        FixtureScope::Test
    }
}

#[derive(Clone)]
pub struct FixtureSetupFn {
    name: String,
    function: fn(TestContext) -> Result<Box<dyn Any + Send + Sync>>,
}

impl FixtureSetupFn {
    pub fn new<S: Into<String>>(
        name: S,
        function: fn(TestContext) -> Result<Box<dyn Any + Send + Sync>>,
    ) -> Self {
        Self {
            name: name.into(),
            function,
        }
    }

    pub fn exec(&self, context: TestContext) -> Result<Box<dyn Any + Send + Sync>> {
        (self.function)(context)
    }
}

impl std::fmt::Debug for FixtureSetupFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixtureSetupFn")
            .field("name", &self.name)
            .field("function", &"<function>")
            .finish()
    }
}

#[derive(Clone)]
pub struct FixtureTeardownFn {
    name: String,
    function: fn(Box<dyn Any + Send + Sync>, TestContext) -> Result<()>,
}

impl FixtureTeardownFn {
    pub fn new<S: Into<String>>(
        name: S,
        function: fn(Box<dyn Any + Send + Sync>, TestContext) -> Result<()>,
    ) -> Self {
        Self {
            name: name.into(),
            function,
        }
    }

    pub fn exec(&self, value: Box<dyn Any + Send + Sync>, context: TestContext) -> Result<()> {
        (self.function)(value, context)
    }
}

impl std::fmt::Debug for FixtureTeardownFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixtureTeardownFn")
            .field("name", &self.name)
            .field("function", &"<function>")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct FixtureDefinition {
    pub id: Uuid,
    pub name: String,
    pub scope: FixtureScope,
    pub required: bool,
    pub is_async: bool,

    // other fixtures that this fixture depends on
    pub dependencies: Vec<String>,
    // function pointer for setting up the fixture
    pub setup: Option<FixtureSetupFn>,
    // function pointer for tearing down the fixture
    pub teardown: Option<FixtureTeardownFn>,

    // misc metadata for the fixture
    pub metadata: HashMap<String, serde_json::Value>,
}

impl FixtureDefinition {
    pub fn new<S: Into<String>>(name: S, scope: FixtureScope) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            scope,
            setup: None,
            teardown: None,
            dependencies: Vec::new(),
            required: true,
            is_async: false,
            metadata: HashMap::new(),
        }
    }

    pub fn with_setup<S: Into<String>>(
        mut self,
        name: S,
        setup: fn(TestContext) -> Result<Box<dyn Any + Send + Sync>>,
    ) -> Self {
        self.setup = Some(FixtureSetupFn::new(name, setup));
        self
    }

    pub fn with_teardown<S: Into<String>>(
        mut self,
        name: S,
        teardown: fn(Box<dyn Any + Send + Sync>, TestContext) -> Result<()>,
    ) -> Self {
        self.teardown = Some(FixtureTeardownFn::new(name, teardown));
        self
    }

    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }

    pub fn depends_on<S: Into<String>>(mut self, fixture_name: S) -> Self {
        self.dependencies.push(fixture_name.into());
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn with_metadata<K, V>(mut self, key: K, value: V) -> Result<Self>
    where
        K: Into<String>,
        V: Serialize,
    {
        let json_value = serde_json::to_value(value)?;
        self.metadata.insert(key.into(), json_value);
        Ok(self)
    }

    pub fn with_async(mut self, is_async: bool) -> Self {
        self.is_async = is_async;
        self
    }
}
