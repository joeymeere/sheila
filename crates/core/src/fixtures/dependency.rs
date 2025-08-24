use crate::{Error, FixtureScope, Result, fixtures::FixtureDefinition, test::TestContext};
use indexmap::{IndexMap, IndexSet};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FixtureDependencyGraph {
    /// fixture name -> dependencies
    dependencies: IndexMap<String, Vec<String>>,
    /// fixture name -> definition
    fixtures: IndexMap<String, FixtureDefinition>,
}

impl FixtureDependencyGraph {
    pub fn new() -> Self {
        Self {
            dependencies: IndexMap::new(),
            fixtures: IndexMap::new(),
        }
    }

    pub fn add_fixture(&mut self, fixture: FixtureDefinition) {
        let name = fixture.name.clone();
        let deps = fixture.dependencies.clone();

        self.dependencies.insert(name.clone(), deps);
        self.fixtures.insert(name, fixture);
    }

    pub fn resolve_order(&self) -> Result<Vec<String>> {
        let mut visited = IndexSet::new();
        let mut temp_visited = IndexSet::new();
        let mut result = Vec::new();

        for fixture_name in self.fixtures.keys() {
            if !visited.contains(fixture_name) {
                self.visit_fixture(fixture_name, &mut visited, &mut temp_visited, &mut result)?;
            }
        }

        Ok(result)
    }

    pub fn get_dependents(&self, fixture_name: &str) -> Vec<String> {
        self.dependencies
            .iter()
            .filter_map(|(name, deps)| {
                if deps.contains(&fixture_name.to_string()) {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn has_circular_dependencies(&self) -> bool {
        self.resolve_order().is_err()
    }

    pub fn get_fixture(&self, name: &str) -> Option<&FixtureDefinition> {
        self.fixtures.get(name)
    }

    pub fn all_names(&self) -> Vec<String> {
        self.fixtures.keys().cloned().collect()
    }

    fn visit_fixture(
        &self,
        fixture_name: &str,
        visited: &mut IndexSet<String>,
        temp_visited: &mut IndexSet<String>,
        result: &mut Vec<String>,
    ) -> Result<()> {
        if temp_visited.contains(fixture_name) {
            return Err(Error::fixture(format!(
                "Circular dependency detected involving fixture '{}'",
                fixture_name
            )));
        }

        if visited.contains(fixture_name) {
            return Ok(());
        }

        temp_visited.insert(fixture_name.to_string());

        if let Some(dependencies) = self.dependencies.get(fixture_name) {
            for dep in dependencies {
                if !self.fixtures.contains_key(dep) {
                    return Err(Error::fixture(format!(
                        "Fixture '{}' depends on undefined fixture '{}'",
                        fixture_name, dep
                    )));
                }
                self.visit_fixture(dep, visited, temp_visited, result)?;
            }
        }

        temp_visited.remove(fixture_name);
        visited.insert(fixture_name.to_string());
        result.push(fixture_name.to_string());

        Ok(())
    }
}

#[derive(Debug)]
pub struct FixtureRegistry {
    graph: FixtureDependencyGraph,
    /// suite-scoped fixture instances
    suite_instances: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
    /// test-scoped fixture instances
    test_instances: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
}

impl FixtureRegistry {
    pub fn new() -> Self {
        Self {
            graph: FixtureDependencyGraph::new(),
            suite_instances: HashMap::new(),
            test_instances: HashMap::new(),
        }
    }

    pub fn register_fixture(&mut self, fixture: FixtureDefinition) {
        self.graph.add_fixture(fixture);
    }

    pub fn setup_suite_fixtures(&mut self, test_context: &crate::test::TestContext) -> Result<()> {
        let fixture_order = self.graph.resolve_order()?;

        for fixture_name in fixture_order {
            if let Some(fixture) = self.graph.get_fixture(&fixture_name) {
                if fixture.scope == super::FixtureScope::Suite {
                    if let Some(ref setup_fn) = fixture.setup {
                        let instance = setup_fn.exec(test_context.clone())?;
                        self.suite_instances.insert(fixture_name, instance);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn setup_test_fixtures(&mut self, test_context: &crate::test::TestContext) -> Result<()> {
        let fixture_order = self.graph.resolve_order()?;
        for fixture_name in fixture_order {
            if let Some(fixture) = self.graph.get_fixture(&fixture_name) {
                if fixture.scope == super::FixtureScope::Test {
                    if let Some(ref setup_fn) = fixture.setup {
                        let instance = setup_fn.exec(test_context.clone())?;
                        self.test_instances.insert(fixture_name, instance);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn teardown_test_fixtures(&mut self, test_context: &TestContext) -> Result<()> {
        let mut fixture_order = self.graph.resolve_order()?;
        fixture_order.reverse();

        for fixture_name in fixture_order {
            if let Some(fixture) = self.graph.get_fixture(&fixture_name) {
                if fixture.scope == FixtureScope::Test {
                    if let Some(instance) = self.test_instances.remove(&fixture_name) {
                        if let Some(ref teardown_fn) = fixture.teardown {
                            teardown_fn.exec(instance, test_context.clone())?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn teardown_suite_fixtures(&mut self, test_context: &TestContext) -> Result<()> {
        let mut fixture_order = self.graph.resolve_order()?;
        fixture_order.reverse();
        for fixture_name in fixture_order {
            if let Some(fixture) = self.graph.get_fixture(&fixture_name) {
                if fixture.scope == super::FixtureScope::Suite {
                    if let Some(instance) = self.suite_instances.remove(&fixture_name) {
                        if let Some(ref teardown_fn) = fixture.teardown {
                            teardown_fn.exec(instance, test_context.clone())?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_fixture_instance<T: 'static>(&self, name: &str) -> Option<&T> {
        if let Some(instance) = self.test_instances.get(name) {
            instance.downcast_ref::<T>()
        } else if let Some(instance) = self.suite_instances.get(name) {
            instance.downcast_ref::<T>()
        } else {
            None
        }
    }
}

impl Default for FixtureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::FixtureScope;

    use super::*;

    #[test]
    fn test_dep_resolution() {
        let mut graph = FixtureDependencyGraph::new();

        // dependencies: C -> B -> A
        graph.add_fixture(FixtureDefinition::new("A", FixtureScope::Test));
        graph.add_fixture(
            FixtureDefinition::new("B", FixtureScope::Test)
                .with_dependencies(vec!["A".to_string()]),
        );
        graph.add_fixture(
            FixtureDefinition::new("C", FixtureScope::Test)
                .with_dependencies(vec!["B".to_string()]),
        );

        let order = graph.resolve_order().unwrap();

        // A should come before B -- B should come before C
        let a_pos = order.iter().position(|x| x == "A").unwrap();
        let b_pos = order.iter().position(|x| x == "B").unwrap();
        let c_pos = order.iter().position(|x| x == "C").unwrap();

        assert!(a_pos < b_pos);
        assert!(b_pos < c_pos);
    }

    #[test]
    fn test_circular_deps() {
        let mut graph = FixtureDependencyGraph::new();

        // circular dependency: A -> B -> A
        graph.add_fixture(
            FixtureDefinition::new("A", FixtureScope::Test)
                .with_dependencies(vec!["B".to_string()]),
        );
        graph.add_fixture(
            FixtureDefinition::new("B", FixtureScope::Test)
                .with_dependencies(vec!["A".to_string()]),
        );

        assert!(graph.has_circular_dependencies());
        assert!(graph.resolve_order().is_err());
    }
}
