# Sheila Proc Macros

This crate provides procedural macros for the Sheila testing framework, enabling ergonomic test definition and configuration.

## Features

- `#[sheila::test]` - Define test functions
- `#[sheila::suite]` - Define test suites
- `#[sheila::fixture]` - Define fixtures for test setup
- Test attributes: `#[ignore]`, `#[only]`, `#[retries(n)]`, `#[timeout(n)]`, `#[tags(...)]`
- Hook attributes: `#[before_all]`, `#[after_all]`, `#[before_each]`, `#[after_each]`
- `#[derive(TestSuite)]` - Automatically implement test suite traits

## Basic Usage

```rust
use sheila_proc_macros as sheila;

// Define a test suite
#[sheila::suite]
struct MathTests;

// Define tests
#[sheila::test]
fn test_addition() {
    assert_eq!(2 + 2, 4);
}

#[sheila::test]
#[sheila::ignore] // Skip this test
fn test_todo() {
    todo!("Implement later");
}

#[sheila::test]
#[sheila::timeout(30)] // 30 second timeout
#[sheila::retries(3)]  // Retry up to 3 times
#[sheila::tags("integration", "slow")]
fn test_database_connection() {
    // Test implementation
}

// Define fixtures
#[sheila::fixture]
fn database() -> Database {
    Database::connect("test_db")
}

// Define hooks
#[sheila::before_all]
fn setup() {
    println!("Setting up test suite");
}

#[sheila::after_all]
fn cleanup() {
    println!("Cleaning up test suite");
}
```

## Test Attributes

### `#[ignore]`
Skip a test during execution.

```rust
#[sheila::test]
#[sheila::ignore]
fn test_not_ready() {
    // This test will be skipped
}
```

### `#[only]`
Run only this test (useful for debugging).

```rust
#[sheila::test]
#[sheila::only]
fn test_focused() {
    // Only this test will run
}
```

### `#[retries(n)]`
Retry a test up to n times on failure.

```rust
#[sheila::test]
#[sheila::retries(3)]
fn test_flaky() {
    // Will retry up to 3 times
}
```

### `#[timeout(seconds)]`
Set a timeout for test execution.

```rust
#[sheila::test]
#[sheila::timeout(60)]
fn test_long_running() {
    // Will timeout after 60 seconds
}
```

### `#[tags(...)]`
Add tags for test filtering and organization.

```rust
#[sheila::test]
#[sheila::tags("unit", "fast")]
fn test_quick_unit() {
    // Tagged as "unit" and "fast"
}
```

## Fixtures

Define reusable test setup with fixtures:

```rust
#[sheila::fixture]
fn temp_dir() -> TempDir {
    TempDir::new().unwrap()
}

#[sheila::fixture]
fn database() -> Database {
    Database::new_test_instance()
}
```

## Hooks

Define lifecycle hooks for test suites:

```rust
#[sheila::before_all]
fn setup_suite() {
    // Run once before all tests in the suite
}

#[sheila::after_all]
fn cleanup_suite() {
    // Run once after all tests in the suite
}

#[sheila::before_each]
fn setup_test() {
    // Run before each individual test
}

#[sheila::after_each]
fn cleanup_test() {
    // Run after each individual test
}
```

## Test Suites

Organize tests into suites:

```rust
#[sheila::suite]
struct DatabaseTests;

impl DatabaseTests {
    // Tests and hooks can be defined as methods
}

// Or use derive for automatic implementation
#[derive(sheila::TestSuite)]
struct ApiTests {
    base_url: String,
}
```

## Generated Code

The macros generate wrapper functions and registration code that integrates with the Sheila test runner. For example:

```rust
#[sheila::test]
fn my_test() {
    assert!(true);
}
```

Generates:

```rust
fn my_test() {
    assert!(true);
}

#[doc(hidden)]
pub fn __sheila_test_my_test() -> sheila::Test {
    let test_fn: sheila::TestFn = Box::new(|_ctx: sheila::TestContext| -> sheila::Result<()> {
        my_test();
        Ok(())
    });
    
    sheila::Test::new("my test", test_fn)
}
```

This allows the Sheila test runner to discover and execute tests automatically.

## Integration

These macros are designed to work seamlessly with the Sheila core framework and CLI tools. Tests defined with these macros can be discovered and executed by the Sheila test runner. 