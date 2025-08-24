use super::*;

use crate::test::TestContext;
use crate::{Error, Result};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct FixtureInstance {
    pub definition: Arc<FixtureDefinition>,
    pub value: Box<dyn Any + Send + Sync>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub context: TestContext,
}

impl FixtureInstance {
    pub fn new(
        definition: Arc<FixtureDefinition>,
        value: Box<dyn Any + Send + Sync>,
        context: TestContext,
    ) -> Self {
        Self {
            definition,
            value,
            created_at: chrono::Utc::now(),
            context,
        }
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.value.downcast_ref::<T>()
    }

    pub fn teardown(self) -> Result<()> {
        if let Some(ref teardown_fn) = self.definition.teardown {
            teardown_fn.exec(self.value, self.context)?;
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct FixtureManager {
    definitions: HashMap<String, Arc<FixtureDefinition>>,
    session_fixtures: HashMap<String, Arc<FixtureInstance>>,
    suite_fixtures: HashMap<String, Arc<FixtureInstance>>,
    test_fixtures: HashMap<String, Arc<FixtureInstance>>,
}

impl FixtureManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, definition: FixtureDefinition) -> Result<()> {
        let name = definition.name.clone();
        self.definitions.insert(name, Arc::new(definition));
        Ok(())
    }

    pub fn get_fixture(&self, name: &str, scope: FixtureScope) -> Option<Arc<FixtureInstance>> {
        match scope {
            FixtureScope::Session => self.session_fixtures.get(name).cloned(),
            FixtureScope::Suite => self.suite_fixtures.get(name).cloned(),
            FixtureScope::Test | FixtureScope::Invocation => self.test_fixtures.get(name).cloned(),
        }
    }

    pub fn setup_fixture(
        &mut self,
        name: &str,
        context: TestContext,
    ) -> Result<Arc<FixtureInstance>> {
        let definition = self
            .definitions
            .get(name)
            .ok_or_else(|| Error::fixture(format!("Fixture '{}' not found", name)))?
            .clone();

        for dep_name in &definition.dependencies {
            if self.get_fixture(dep_name, definition.scope).is_none() {
                self.setup_fixture(dep_name, context.clone())?;
            }
        }

        let value = if let Some(ref setup_fn) = definition.setup {
            setup_fn.exec(context.clone())?
        } else {
            Box::new(())
        };

        let instance = Arc::new(FixtureInstance::new(definition.clone(), value, context));
        match definition.scope {
            FixtureScope::Session => {
                self.session_fixtures
                    .insert(name.to_string(), instance.clone());
            }
            FixtureScope::Suite => {
                self.suite_fixtures
                    .insert(name.to_string(), instance.clone());
            }
            FixtureScope::Test | FixtureScope::Invocation => {
                self.test_fixtures
                    .insert(name.to_string(), instance.clone());
            }
        }

        Ok(instance)
    }

    pub fn teardown_by_scope(&mut self, scope: FixtureScope) -> Result<()> {
        let fixtures_to_teardown = match scope {
            FixtureScope::Session => {
                let fixtures: Vec<_> = self.session_fixtures.drain().collect();
                fixtures
            }
            FixtureScope::Suite => {
                let fixtures: Vec<_> = self.suite_fixtures.drain().collect();
                fixtures
            }
            FixtureScope::Test | FixtureScope::Invocation => {
                let fixtures: Vec<_> = self.test_fixtures.drain().collect();
                fixtures
            }
        };

        for (_name, instance) in fixtures_to_teardown {
            if let Ok(instance) = Arc::try_unwrap(instance) {
                instance.teardown()?;
            }
        }

        Ok(())
    }

    pub fn all_names(&self) -> Vec<String> {
        self.definitions.keys().cloned().collect()
    }

    pub fn has_fixture(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }
}
