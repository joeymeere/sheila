pub mod dependency;
pub mod lifecycle;
pub mod scope;

pub use dependency::*;
pub use lifecycle::*;
pub use scope::*;

use crate::test::TestContext;
use crate::{Error, Result};
use std::any::Any;

pub trait Fixture: Send + Sync + 'static {
    type Output: Send + Sync + 'static;

    /// Setup the fixture
    fn setup(context: TestContext) -> Result<Self::Output>;

    /// Teardown the fixture (default: no-op)
    fn teardown(_value: Self::Output, _context: TestContext) -> Result<()> {
        Ok(())
    }

    /// Get fixture scope (default: `FixtureScope::Test`)
    fn scope() -> FixtureScope {
        FixtureScope::Test
    }

    /// Get fixture dependencies (default: `vec![]`)
    fn dependencies() -> Vec<String> {
        Vec::new()
    }

    /// Convert to `FixtureDefinition`
    fn definition<S: Into<String>>(name: S) -> FixtureDefinition {
        let name = name.into();

        let setup_fn = |context: TestContext| -> Result<Box<dyn Any + Send + Sync>> {
            let value = Self::setup(context)?;
            let boxed: Box<dyn Any + Send + Sync> = Box::new(value);
            Ok(boxed)
        };

        let teardown_fn = |value: Box<dyn Any + Send + Sync>, context: TestContext| -> Result<()> {
            if let Ok(typed_value) = value.downcast::<Self::Output>() {
                Self::teardown(*typed_value, context)
            } else {
                Err(Error::fixture("Failed to downcast fixture for teardown"))
            }
        };

        FixtureDefinition::new(name.clone(), Self::scope())
            .with_setup(format!("{}_setup", name), setup_fn)
            .with_teardown(format!("{}_teardown", name), teardown_fn)
    }
}
