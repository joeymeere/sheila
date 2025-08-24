//region MOCKING
#[macro_export]
macro_rules! mock_fn {
    ($name:expr, $config:expr) => {
        $crate::mock::global::mock($name, $config)
    };
}

#[macro_export]
macro_rules! mock_call {
    ($name:expr, $($arg:expr),* $(,)?) => {
        {
            let args = vec![$(serde_json::to_value($arg).unwrap()),*];
            $crate::mock::global::call($name, args)
        }
    };
}

#[macro_export]
macro_rules! expect_calls {
    ($name:expr, $count:expr) => {
        $crate::mock::MockBuilder::new()
            .expect_calls($count)
            .build()
    };
}

#[macro_export]
macro_rules! returns {
    ($value:expr) => {
        $crate::mock::MockBuilder::new()
            .returns($value)
            .unwrap()
            .build()
    };
}
//endregion

//region PARAMETERIZATION
#[macro_export]
macro_rules! params {
    ($($key:expr => $value:expr),* $(,)?) => {
        {
            let mut set = $crate::parameterize::ParameterSet::new();
            $(
                set = set.with_param($key, $value).unwrap();
            )*
            set
        }
    };
}

#[macro_export]
macro_rules! param_sets {
    ($($params:expr),* $(,)?) => {
        {
            let mut collection = $crate::parameterize::ParameterCollection::new();
            $(
                collection = collection.add_set($params);
            )*
            collection
        }
    };
}
//endregion

//region RESULTS & ASSERTIONS
#[macro_export]
macro_rules! fail {
    ($message:expr) => {
        $crate::result::Result::Err($crate::result::Error::intended_failure($message))
    };
}

#[macro_export]
macro_rules! test_result {
    ($expr:expr) => {
        $expr.into_test_error()
    };
}

#[macro_export]
macro_rules! fixture_result {
    ($expr:expr) => {
        $expr.into_fixture_error()
    };
}

#[macro_export]
macro_rules! assertion_result {
    ($expr:expr) => {
        $expr.into_assertion_error()
    };
}

#[macro_export]
macro_rules! assert_that {
    ($value:expr, $predicate:expr, $message:expr) => {
        $crate::assertion::Assertion::that($value, $predicate, $message)
    };
}

#[macro_export]
macro_rules! assert_eq {
    ($expected:expr, $actual:expr) => {
        $crate::assertion::Assertion::eq($expected, $actual)
    };
}

#[macro_export]
macro_rules! assert_ne {
    ($expected:expr, $actual:expr) => {
        $crate::assertion::Assertion::ne($expected, $actual)
    };
}

#[macro_export]
macro_rules! assert_true {
    ($value:expr) => {
        $crate::assertion::Assertion::is_true($value)
    };
}

#[macro_export]
macro_rules! assert_false {
    ($value:expr) => {
        $crate::assertion::Assertion::is_false($value)
    };
}

#[macro_export]
macro_rules! assert_some {
    ($value:expr) => {
        $crate::assertion::Assertion::is_some($value)
    };
}

#[macro_export]
macro_rules! assert_none {
    ($value:expr) => {
        $crate::assertion::Assertion::is_none($value)
    };
}

#[macro_export]
macro_rules! assert_ok {
    ($value:expr) => {
        $crate::assertion::Assertion::is_ok($value)
    };
}

#[macro_export]
macro_rules! assert_err {
    ($value:expr) => {
        $crate::assertion::Assertion::is_err($value)
    };
}

#[macro_export]
macro_rules! assert_contains {
    ($haystack:expr, $needle:expr) => {
        $crate::assertion::Assertion::contains($haystack, $needle)
    };
}

#[macro_export]
macro_rules! assert_empty {
    ($collection:expr) => {
        $crate::assertion::Assertion::is_empty($collection)
    };
}

#[macro_export]
macro_rules! assert_not_empty {
    ($collection:expr) => {
        $crate::assertion::Assertion::is_not_empty($collection)
    };
}

#[macro_export]
macro_rules! assert_length {
    ($collection:expr, $length:expr) => {
        $crate::assertion::Assertion::has_length($collection, $length)
    };
}

#[macro_export]
macro_rules! assert_approx_eq {
    ($actual:expr, $expected:expr, $epsilon:expr) => {
        $crate::assertion::Assertion::approx_eq($actual, $expected, $epsilon)
    };
}
//endregion

//region DEBUG
#[macro_export]
macro_rules! debug_log {
    ($ctx:expr, $level:ident, $($arg:tt)*) => {
        match stringify!($level) {
            "info" => $ctx.info(format!($($arg)*)),
            "debug" => $ctx.debug(format!($($arg)*)),
            "warn" => $ctx.warn(format!($($arg)*)),
            "error" => $ctx.error(format!($($arg)*)),
            _ => $ctx.debug(format!($($arg)*)),
        }
    };
}

#[macro_export]
macro_rules! breadcrumb {
    ($ctx:expr, $($arg:tt)*) => {
        $ctx.add_breadcrumb(format!($($arg)*))
    };
}
//endregion
