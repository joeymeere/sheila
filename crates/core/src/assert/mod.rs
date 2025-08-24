use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::fmt::{Debug, Display};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    /// Whether the assertion passed
    pub passed: bool,
    /// Expected value (if applicable)
    pub expected: Option<String>,
    /// Actual value (if applicable)
    pub actual: Option<String>,
    /// Assertion message
    pub message: String,
    /// Additional context
    pub context: Vec<String>,
    /// Diff information (for string comparisons)
    pub diff: Option<String>,
}

impl AssertionResult {
    /// Create a passing assertion result
    pub fn pass(message: String) -> Self {
        Self {
            passed: true,
            expected: None,
            actual: None,
            message,
            context: Vec::new(),
            diff: None,
        }
    }

    /// Create a failing assertion result
    pub fn fail(message: String) -> Self {
        Self {
            passed: false,
            expected: None,
            actual: None,
            message,
            context: Vec::new(),
            diff: None,
        }
    }

    /// Create a failing assertion result with expected and actual values
    pub fn fail_with_values<E, A>(message: String, expected: E, actual: A) -> Self
    where
        E: Display,
        A: Display,
    {
        let expected_str = expected.to_string();
        let actual_str = actual.to_string();

        let diff = if expected_str.contains('\n') || actual_str.contains('\n') {
            Some(create_diff(&expected_str, &actual_str))
        } else {
            None
        };

        Self {
            passed: false,
            expected: Some(expected_str),
            actual: Some(actual_str),
            message,
            context: Vec::new(),
            diff,
        }
    }

    /// Add context to the assertion result
    pub fn with_context<S: Into<String>>(mut self, context: S) -> Self {
        self.context.push(context.into());
        self
    }

    /// Convert to Result
    pub fn into_result(self) -> Result<()> {
        if self.passed {
            Ok(())
        } else {
            let mut message = self.message;

            if let (Some(expected), Some(actual)) = (&self.expected, &self.actual) {
                message.push_str(&format!("\nExpected: {}\nActual: {}", expected, actual));
            }

            if let Some(diff) = &self.diff {
                message.push_str(&format!("\nDiff:\n{}", diff));
            }

            for context in &self.context {
                message.push_str(&format!("\nContext: {}", context));
            }

            Err(Error::assertion(message))
        }
    }
}

fn create_diff(expected: &str, actual: &str) -> String {
    let diff = TextDiff::from_lines(expected, actual);
    let mut result = String::new();

    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        result.push_str(&format!("{}{}", sign, change));
    }

    result
}

pub struct Assertion;

impl Assertion {
    pub fn new() -> Self {
        Self
    }

    /// Assert that a value is true
    pub fn is_true(value: bool) -> Result<()> {
        if value {
            AssertionResult::pass("Value is true".to_string()).into_result()
        } else {
            AssertionResult::fail_with_values("Expected true".to_string(), true, value)
                .into_result()
        }
    }

    /// Assert that a value is false
    pub fn is_false(value: bool) -> Result<()> {
        if !value {
            AssertionResult::pass("Value is false".to_string()).into_result()
        } else {
            AssertionResult::fail_with_values("Expected false".to_string(), false, value)
                .into_result()
        }
    }

    /// Assert that two values are equal
    pub fn eq<T>(expected: T, actual: T) -> Result<()>
    where
        T: PartialEq + Debug + Display,
    {
        if expected == actual {
            AssertionResult::pass("Values are equal".to_string()).into_result()
        } else {
            AssertionResult::fail_with_values("Values are not equal".to_string(), expected, actual)
                .into_result()
        }
    }

    /// Assert that two values are not equal
    pub fn ne<T>(expected: T, actual: T) -> Result<()>
    where
        T: PartialEq + Debug + Display,
    {
        if expected != actual {
            AssertionResult::pass("Values are not equal".to_string()).into_result()
        } else {
            AssertionResult::fail_with_values(
                "Values should not be equal".to_string(),
                expected,
                actual,
            )
            .into_result()
        }
    }

    /// Assert that a value is greater than another
    pub fn gt<T>(actual: T, expected: T) -> Result<()>
    where
        T: PartialOrd + Debug + Display,
    {
        if actual > expected {
            AssertionResult::pass("Value is greater than expected".to_string()).into_result()
        } else {
            AssertionResult::fail_with_values(
                "Expected value to be greater".to_string(),
                format!(">{}", expected),
                actual,
            )
            .into_result()
        }
    }

    /// Assert that a value is greater than or equal to another
    pub fn ge<T>(actual: T, expected: T) -> Result<()>
    where
        T: PartialOrd + Debug + Display,
    {
        if actual >= expected {
            AssertionResult::pass("Value is greater than or equal to expected".to_string())
                .into_result()
        } else {
            AssertionResult::fail_with_values(
                "Expected value to be greater than or equal".to_string(),
                format!(">={}", expected),
                actual,
            )
            .into_result()
        }
    }

    /// Assert that a value is less than another
    pub fn lt<T>(actual: T, expected: T) -> Result<()>
    where
        T: PartialOrd + Debug + Display,
    {
        if actual < expected {
            AssertionResult::pass("Value is less than expected".to_string()).into_result()
        } else {
            AssertionResult::fail_with_values(
                "Expected value to be less".to_string(),
                format!("<{}", expected),
                actual,
            )
            .into_result()
        }
    }

    /// Assert that a value is less than or equal to another
    pub fn le<T>(actual: T, expected: T) -> Result<()>
    where
        T: PartialOrd + Debug + Display,
    {
        if actual <= expected {
            AssertionResult::pass("Value is less than or equal to expected".to_string())
                .into_result()
        } else {
            AssertionResult::fail_with_values(
                "Expected value to be less than or equal".to_string(),
                format!("<={}", expected),
                actual,
            )
            .into_result()
        }
    }

    /// Assert that a value is None
    pub fn is_none<T>(value: Option<T>) -> Result<()>
    where
        T: Debug,
    {
        match value {
            None => AssertionResult::pass("Value is None".to_string()).into_result(),
            Some(v) => AssertionResult::fail_with_values(
                "Expected None".to_string(),
                "None".to_string(),
                format!("Some({:?})", v),
            )
            .into_result(),
        }
    }

    /// Assert that a value is Some
    pub fn is_some<T>(value: Option<T>) -> Result<()>
    where
        T: Debug,
    {
        match value {
            Some(_) => AssertionResult::pass("Value is Some".to_string()).into_result(),
            None => AssertionResult::fail_with_values(
                "Expected Some".to_string(),
                "Some(_".to_string(),
                "None".to_string(),
            )
            .into_result(),
        }
    }

    /// Assert that a Result is Ok
    pub fn is_ok<T, E>(value: std::result::Result<T, E>) -> Result<()>
    where
        T: Debug,
        E: Debug,
    {
        match value {
            Ok(_) => AssertionResult::pass("Result is Ok".to_string()).into_result(),
            Err(e) => AssertionResult::fail_with_values(
                "Expected Ok".to_string(),
                "Ok(_".to_string(),
                format!("Err({:?})", e),
            )
            .into_result(),
        }
    }

    /// Assert that a Result is Err
    pub fn is_err<T, E>(value: std::result::Result<T, E>) -> Result<()>
    where
        T: Debug,
        E: Debug,
    {
        match value {
            Err(_) => AssertionResult::pass("Result is Err".to_string()).into_result(),
            Ok(v) => AssertionResult::fail_with_values(
                "Expected Err".to_string(),
                "Err(_".to_string(),
                format!("Ok({:?})", v),
            )
            .into_result(),
        }
    }

    /// Assert that a string contains a substring
    pub fn contains(haystack: &str, needle: &str) -> Result<()> {
        if haystack.contains(needle) {
            AssertionResult::pass(format!("String contains '{}'", needle)).into_result()
        } else {
            AssertionResult::fail_with_values(
                format!("String should contain '{}'", needle),
                format!("string containing '{}'", needle),
                haystack,
            )
            .into_result()
        }
    }

    /// Assert that a string starts with a prefix
    pub fn starts_with(haystack: &str, prefix: &str) -> Result<()> {
        if haystack.starts_with(prefix) {
            AssertionResult::pass(format!("String starts with '{}'", prefix)).into_result()
        } else {
            AssertionResult::fail_with_values(
                format!("String should start with '{}'", prefix),
                format!("string starting with '{}'", prefix),
                haystack,
            )
            .into_result()
        }
    }

    /// Assert that a string ends with a suffix
    pub fn ends_with(haystack: &str, suffix: &str) -> Result<()> {
        if haystack.ends_with(suffix) {
            AssertionResult::pass(format!("String ends with '{}'", suffix)).into_result()
        } else {
            AssertionResult::fail_with_values(
                format!("String should end with '{}'", suffix),
                format!("string ending with '{}'", suffix),
                haystack,
            )
            .into_result()
        }
    }

    /// Assert that a string matches a regex pattern
    #[cfg(feature = "regex")]
    pub fn matches(haystack: &str, pattern: &str) -> Result<()> {
        use regex::Regex;

        let regex = Regex::new(pattern)
            .map_err(|e| Error::assertion(format!("Invalid regex pattern '{}': {}", pattern, e)))?;

        if regex.is_match(haystack) {
            AssertionResult::pass(format!("String matches pattern '{}'", pattern)).into_result()
        } else {
            AssertionResult::fail_with_values(
                format!("String should match pattern '{}'", pattern),
                format!("string matching '{}'", pattern),
                haystack,
            )
            .into_result()
        }
    }

    /// Assert that a collection is empty
    pub fn is_empty<T>(collection: &[T]) -> Result<()> {
        if collection.is_empty() {
            AssertionResult::pass("Collection is empty".to_string()).into_result()
        } else {
            AssertionResult::fail_with_values(
                "Collection should be empty".to_string(),
                "empty collection",
                format!("collection with {} items", collection.len()),
            )
            .into_result()
        }
    }

    /// Assert that a collection is not empty
    pub fn is_not_empty<T>(collection: &[T]) -> Result<()> {
        if !collection.is_empty() {
            AssertionResult::pass("Collection is not empty".to_string()).into_result()
        } else {
            AssertionResult::fail_with_values(
                "Collection should not be empty".to_string(),
                "non-empty collection",
                "empty collection",
            )
            .into_result()
        }
    }

    /// Assert that a collection has a specific length
    pub fn has_length<T>(collection: &[T], expected_length: usize) -> Result<()> {
        let actual_length = collection.len();
        if actual_length == expected_length {
            AssertionResult::pass(format!("Collection has length {}", expected_length))
                .into_result()
        } else {
            AssertionResult::fail_with_values(
                "Collection has wrong length".to_string(),
                expected_length,
                actual_length,
            )
            .into_result()
        }
    }

    /// Assert that a collection contains an item
    pub fn contains_item<T>(collection: &[T], item: &T) -> Result<()>
    where
        T: PartialEq + Debug,
    {
        if collection.contains(item) {
            AssertionResult::pass(format!("Collection contains {:?}", item)).into_result()
        } else {
            AssertionResult::fail_with_values(
                "Collection should contain item".to_string(),
                format!("collection containing {:?}", item),
                format!("collection: {:?}", collection),
            )
            .into_result()
        }
    }

    pub fn approx_eq(actual: f64, expected: f64, epsilon: f64) -> Result<()> {
        let diff = (actual - expected).abs();
        if diff <= epsilon {
            AssertionResult::pass("Values are approximately equal".to_string()).into_result()
        } else {
            AssertionResult::fail_with_values(
                format!(
                    "Values are not approximately equal (diff: {}, epsilon: {})",
                    diff, epsilon
                ),
                expected,
                actual,
            )
            .into_result()
        }
    }

    pub fn that<T, F>(value: T, predicate: F, message: &str) -> Result<()>
    where
        T: Debug,
        F: FnOnce(&T) -> bool,
    {
        if predicate(&value) {
            AssertionResult::pass(format!("Custom assertion passed: {}", message)).into_result()
        } else {
            AssertionResult::fail_with_values(
                format!("Custom assertion failed: {}", message),
                message,
                format!("{:?}", value),
            )
            .into_result()
        }
    }
}

impl Default for Assertion {
    fn default() -> Self {
        Self::new()
    }
}
