#![feature(mpmc_channel)]
#![feature(duration_millis_float)]

pub mod assert;
pub mod fixtures;
pub mod internal;
pub mod macros;
pub mod misc;
pub mod reporting;
pub mod result;
pub mod runners;
pub mod schemas;
pub mod suite;
pub mod test;
pub mod types;

pub use assert::Assertion;
pub use fixtures::{Fixture, FixtureScope};
pub use internal::{Hook, HookType, Hooks, MockCollection, MockConfig, ParameterSet};
pub use misc::*;
pub use reporting::{ReportFormat, Reporter, TestReport};
pub use result::{Error, ErrorKind, Result};
pub use runners::{RunnerConfig, TestRunner};
pub use suite::{SuiteAttributes, TestSuite};
pub use test::{Test, TestAttributes, TestFn, TestMetadata, TestStatus};
pub use types::*;

#[cfg(feature = "macros")]
pub use sheila_proc_macros::*;

pub mod prelude {
    pub use crate::{
        Assertion, Error, ErrorKind, Fixture, FixtureScope, Hook, HookType, Hooks, ReportFormat,
        Reporter, Result, RunnerConfig, SuiteAttributes, Test, TestAttributes, TestFn,
        TestMetadata, TestReport, TestRunner, TestStatus, TestSuite, test::TestContext,
    };
    pub use crate::{
        assert_approx_eq, assert_contains, assert_empty, assert_eq, assert_err, assert_false,
        assert_length, assert_ne, assert_none, assert_not_empty, assertion_result, breadcrumb,
        debug_log, expect_calls, mock_call, mock_fn, param_sets, params, returns,
    };
    pub use chrono::{DateTime, Utc};
    pub use indexmap::IndexMap;
    pub use serde::{Deserialize, Serialize};
    pub use uuid::Uuid;
}

use std::path::PathBuf;

pub fn format_relative_path(path: &PathBuf) -> String {
    let current = std::env::current_dir().unwrap();
    path.strip_prefix(&current)
        .unwrap()
        .to_string_lossy()
        .to_string()
}
