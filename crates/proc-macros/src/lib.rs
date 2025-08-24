use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::{Attribute, DeriveInput, Item, ItemFn, ItemMod, parse_macro_input};

#[proc_macro_attribute]
pub fn test(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();
    let test_name = fn_name_str.replace('_', " ");

    let wrapper_name = syn::Ident::new(&format!("__sheila_test_{}", fn_name), fn_name.span());
    let cargo_test_name = syn::Ident::new(&format!("{}_cargo_test", fn_name), fn_name.span());

    let mut ignore = false;
    let mut only = false;
    let mut retries = 0u32;
    let mut timeout_seconds = 0u64;
    let mut tags = Vec::<String>::new();

    for attr in &input_fn.attrs {
        if attr.path().is_ident("ignore") {
            ignore = true;
        } else if attr.path().is_ident("only") {
            only = true;
        } else if attr.path().is_ident("retries") {
            let meta_str = attr.meta.to_token_stream().to_string();
            if let Some(num_str) = meta_str
                .strip_prefix("retries (")
                .and_then(|s| s.strip_suffix(')'))
            {
                retries = num_str.trim().parse().unwrap_or(0);
            }
        } else if attr.path().is_ident("timeout") {
            let meta_str = attr.meta.to_token_stream().to_string();
            if let Some(num_str) = meta_str
                .strip_prefix("timeout (")
                .and_then(|s| s.strip_suffix(')'))
            {
                timeout_seconds = num_str.trim().parse().unwrap_or(0);
            }
        } else if attr.path().is_ident("tags") {
            let meta_str = attr.meta.to_token_stream().to_string();
            if let Some(inner) = meta_str
                .strip_prefix("tags (")
                .and_then(|s| s.strip_suffix(')'))
            {
                tags = inner
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }

    let cargo_test_ignore = if ignore {
        quote! { #[ignore] }
    } else {
        quote! {}
    };

    let output_fn = if cfg!(feature = "__sheila_test") {
        quote! {
            #[test]
            #cargo_test_ignore
            #[allow(non_snake_case)]
            fn #cargo_test_name() {
                #fn_name();
            }
        }
    } else if cfg!(feature = "cargo-test") {
        quote! {
            #[test]
            #[allow(non_snake_case)]
            fn #cargo_test_name() {
                #fn_name();
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #input_fn

        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub fn #wrapper_name() -> ::sheila::prelude::Test {
            let test_fn: ::sheila::prelude::TestFn = Box::new(|_ctx: ::sheila::prelude::TestContext| -> ::sheila::prelude::Result<()> {
                #fn_name();
                Ok(())
            });

            let mut test = ::sheila::prelude::Test::new(#test_name, test_fn);

            test.attributes.ignore = #ignore;
            test.attributes.only = #only;
            test.attributes.retries = #retries;

            if #timeout_seconds > 0 {
                test.attributes.timeout = Some(std::time::Duration::from_secs(#timeout_seconds));
            }

            #(test.metadata.tags.push(#tags.to_string());)*

            test
        }

        #output_fn
    };

    expanded.into()
}

/// Define a test suite with Sheila
///
/// # Basic Usage
/// ```ignore
/// #[sheila::suite]
/// mod my_test_suite {
///     #[sheila::test]
///     fn test_something() {
///         assert!(true);
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn suite(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut input_mod = parse_macro_input!(input as ItemMod);
    let mod_name = &input_mod.ident;
    let mod_name_str = mod_name.to_string();
    let mod_vis = &input_mod.vis;
    let mod_attrs = &input_mod.attrs;

    let module_path = format!("{}::{}", env!("CARGO_PKG_NAME"), mod_name_str);

    if let Some((_brace, ref mut items)) = input_mod.content {
        let discovered = discover_sheila_items(&items);

        let test_registrations = generate_test_registrations(&discovered.tests);
        let fixture_registrations = generate_fixture_registrations(&discovered.fixtures);
        let hook_registrations = generate_hook_registrations(&discovered.hooks);

        items.push(syn::parse_quote! {
            pub fn suite() -> ::sheila::TestSuite {
                ::sheila::TestSuite::new_with_module(#mod_name_str, #module_path)
            }
        });

        items.push(syn::parse_quote! {
            pub fn module_path() -> &'static str {
                #module_path
            }
        });

        items.push(syn::parse_quote! {
            pub fn build_suite() -> ::sheila::TestSuite {
                let mut suite = suite();

                #(#test_registrations)*
                #(#fixture_registrations)*
                #(#hook_registrations)*

                suite
            }
        });

        let stub_mod = syn::Ident::new(&format!("__sheila_{}", mod_name), mod_name.span());
        let expanded = quote! {
            #(#mod_attrs)*
            #mod_vis mod #stub_mod {
                #(#items)*
            }
        };

        expanded.into()
    } else {
        syn::Error::new_spanned(
            input_mod,
            "The #[suite] attribute only works with inline modules",
        )
        .to_compile_error()
        .into()
    }
}

/// Define a fixture with Sheila
///
/// # Basic Usage
/// ```ignore
/// #[sheila::fixture]
/// fn my_fixture() -> String {
///     "test data".to_string()
/// }
/// ```
#[proc_macro_attribute]
pub fn fixture(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();

    let setup_fn_name = syn::Ident::new(
        &format!("__sheila_fixture_setup_{}", fn_name),
        fn_name.span(),
    );
    let registration_fn_name =
        syn::Ident::new(&format!("__sheila_fixture_{}", fn_name), fn_name.span());

    let expanded = quote! {
        #input_fn

        #[doc(hidden)]
        fn #setup_fn_name(_ctx: ::sheila::prelude::TestContext) -> ::sheila::prelude::Result<Box<dyn std::any::Any + Send + Sync>> {
            let result = #fn_name();
            Ok(Box::new(result))
        }

        #[doc(hidden)]
        pub fn #registration_fn_name() -> ::sheila::fixtures::FixtureDefinition {
            ::sheila::fixtures::FixtureDefinition::new(#fn_name_str, ::sheila::fixtures::FixtureScope::Test)
                .with_setup(#fn_name_str, #setup_fn_name)
        }
    };

    expanded.into()
}

/// Set the number of retries for a test
///
/// # Usage
/// ```ignore
/// #[sheila::test]
/// #[sheila::retries(3)]
/// fn flaky_test() {
///     // test code
/// }
/// ```
#[proc_macro_attribute]
pub fn retries(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let retry_count = parse_attribute_args(args)
        .first()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);

    let expanded = quote! {
        #[retries(#retry_count)]
        #input_fn
    };

    expanded.into()
}

/// Set a timeout for a test
///
/// # Usage
/// ```ignore
/// #[sheila::test]
/// #[sheila::timeout(30)]
/// fn slow_test() {
///     // test code
/// }
/// ```
#[proc_macro_attribute]
pub fn timeout(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let timeout_secs = parse_attribute_args(args)
        .first()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let expanded = quote! {
        #[timeout(#timeout_secs)]
        #input_fn
    };

    expanded.into()
}

/// Add tags to a test
///
/// # Usage
/// ```ignore
/// #[sheila::test]
/// #[sheila::tags("api", "integration", "slow")]
/// fn integration_test() {
///     // test code
/// }
/// ```
#[proc_macro_attribute]
pub fn tags(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let tag_list = parse_attribute_args(args);

    let expanded = quote! {
        #[tags(#(#tag_list),*)]
        #input_fn
    };

    expanded.into()
}

/// Add parameters to a test for parameterized testing
///
/// # Usage
/// ```ignore
/// #[sheila::test]
/// #[sheila::params("value1", "value2", "value3")]
/// fn parameterized_test(param: &str) {
///     // test code using param
/// }
/// ```
#[proc_macro_attribute]
pub fn params(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let param_list = parse_attribute_args(args);

    let expanded = quote! {
        #[params(#(#param_list),*)]
        #input_fn
    };

    expanded.into()
}

#[proc_macro_attribute]
pub fn before_all(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();
    let hook_fn_name = syn::Ident::new(&format!("__sheila_before_all_{}", fn_name), fn_name.span());

    let expanded = quote! {
        #input_fn

        #[doc(hidden)]
        pub fn #hook_fn_name() -> ::sheila::internal::Hook {
            ::sheila::internal::Hook::new(
                ::sheila::internal::HookType::BeforeAll,
                #fn_name_str,
                |_ctx: ::sheila::prelude::TestContext| -> ::sheila::prelude::Result<()> {
                    #fn_name();
                    Ok(())
                }
            )
        }
    };

    expanded.into()
}

#[proc_macro_attribute]
pub fn after_all(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();
    let hook_fn_name = syn::Ident::new(&format!("__sheila_after_all_{}", fn_name), fn_name.span());

    let expanded = quote! {
        #input_fn

        #[doc(hidden)]
        pub fn #hook_fn_name() -> ::sheila::internal::Hook {
            ::sheila::internal::Hook::new(
                ::sheila::internal::HookType::AfterAll,
                #fn_name_str,
                |_ctx: ::sheila::prelude::TestContext| -> ::sheila::prelude::Result<()> {
                    #fn_name();
                    Ok(())
                }
            )
        }
    };

    expanded.into()
}

#[proc_macro_attribute]
pub fn before_each(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();
    let hook_fn_name =
        syn::Ident::new(&format!("__sheila_before_each_{}", fn_name), fn_name.span());

    let expanded = quote! {
        #input_fn

        #[doc(hidden)]
        pub fn #hook_fn_name() -> ::sheila::internal::Hook {
            ::sheila::internal::Hook::new(
                ::sheila::internal::HookType::BeforeEach,
                #fn_name_str,
                |_ctx: ::sheila::prelude::TestContext| -> ::sheila::prelude::Result<()> {
                    #fn_name();
                    Ok(())
                }
            )
        }
    };

    expanded.into()
}

#[proc_macro_attribute]
pub fn after_each(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(input as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();
    let hook_fn_name = syn::Ident::new(&format!("__sheila_after_each_{}", fn_name), fn_name.span());

    let expanded = quote! {
        #input_fn

        #[doc(hidden)]
        pub fn #hook_fn_name() -> ::sheila::internal::Hook {
            ::sheila::internal::Hook::new(
                ::sheila::internal::HookType::AfterEach,
                #fn_name_str,
                |_ctx: ::sheila::prelude::TestContext| -> ::sheila::prelude::Result<()> {
                    #fn_name();
                    Ok(())
                }
            )
        }
    };

    expanded.into()
}

#[proc_macro_derive(TestSuite)]
pub fn derive_test_suite(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let struct_name_str = struct_name.to_string();

    let expanded = quote! {
        impl #struct_name {
            pub fn suite() -> sheila::TestSuite {
                sheila::TestSuite::new(#struct_name_str)
            }
        }
    };

    expanded.into()
}

#[derive(Default)]
struct DiscoveredItems {
    tests: Vec<TestInfo>,
    fixtures: Vec<FixtureInfo>,
    hooks: Vec<HookInfo>,
}

struct TestInfo {
    name: String,
    fn_ident: syn::Ident,
    tags: Vec<String>,
}

struct FixtureInfo {
    name: String,
    fn_ident: syn::Ident,
    scope: String,
    depends_on: Vec<String>,
}

struct HookInfo {
    name: String,
    fn_ident: syn::Ident,
    hook_type: HookType,
}

enum HookType {
    BeforeAll,
    AfterAll,
    BeforeEach,
    AfterEach,
}

enum SheilaAttribute {
    Test {
        tags: Vec<String>,
    },
    Fixture {
        scope: String,
        depends_on: Vec<String>,
    },
    Hook(HookType),
}

fn discover_sheila_items(items: &[Item]) -> DiscoveredItems {
    let mut discovered = DiscoveredItems::default();

    for item in items {
        if let Item::Fn(func) = item {
            for attr in &func.attrs {
                if let Some(sheila_attr) = parse_sheila_attribute(attr) {
                    match sheila_attr {
                        SheilaAttribute::Test { tags } => {
                            discovered.tests.push(TestInfo {
                                name: func.sig.ident.to_string(),
                                fn_ident: func.sig.ident.clone(),
                                tags,
                            });
                        }
                        SheilaAttribute::Fixture { scope, depends_on } => {
                            discovered.fixtures.push(FixtureInfo {
                                name: func.sig.ident.to_string(),
                                fn_ident: func.sig.ident.clone(),
                                scope,
                                depends_on,
                            });
                        }
                        SheilaAttribute::Hook(hook_type) => {
                            discovered.hooks.push(HookInfo {
                                name: func.sig.ident.to_string(),
                                fn_ident: func.sig.ident.clone(),
                                hook_type,
                            });
                        }
                    }
                } else {
                    println!("Warning: Unknown sheila attribute: {:?}", attr.path());
                }
            }
        }
    }

    discovered
}

fn generate_test_registrations(tests: &[TestInfo]) -> Vec<TokenStream2> {
    tests
        .iter()
        .map(|test| {
            let fn_ident = &test.fn_ident;
            let test_name = &test.name;

            if test.tags.is_empty() {
                quote! {
                    suite = suite.add_test(::sheila::Test::new(
                        #test_name,
                        |_ctx| {
                            #fn_ident();
                            Ok(())
                        }
                    ));
                }
            } else {
                let tags = &test.tags;
                quote! {
                    suite = suite.add_test(::sheila::Test::new(
                        #test_name,
                        |_ctx| {
                            #fn_ident();
                            Ok(())
                        }
                    ).with_attributes(::sheila::TestAttributes {
                        tags: vec![#(#tags.to_string()),*],
                        ..Default::default()
                    }));
                }
            }
        })
        .collect()
}

fn generate_fixture_registrations(fixtures: &[FixtureInfo]) -> Vec<TokenStream2> {
    fixtures
        .iter()
        .map(|fixture| {
            let fn_ident = &fixture.fn_ident;
            let fixture_name = &fixture.name;
            let scope = &fixture.scope;
            let scope_ident = match scope.as_str() {
                "suite" => syn::Ident::new("Suite", proc_macro2::Span::call_site()),
                "test" => syn::Ident::new("Test", proc_macro2::Span::call_site()),
                _ => syn::Ident::new("Test", proc_macro2::Span::call_site()), // default
            };
            let fixture_scope = quote! { ::sheila::fixtures::FixtureScope::#scope_ident };

            let deps = &fixture.depends_on;
            quote! {
                {
                    let mut fixture_def = ::sheila::fixtures::FixtureDefinition::new(
                        #fixture_name,
                        #fixture_scope
                    );
                    fixture_def.dependencies = vec![#(#deps.to_string()),*];
                    fixture_def.setup = Some(::sheila::fixtures::FixtureSetupFn::new(
                        #fixture_name,
                        |_ctx| {
                            let result = #fn_ident();
                            Ok(Box::new(result))
                        }
                    ));
                    suite.fixtures.register_fixture(fixture_def);
                }
            }
        })
        .collect()
}

fn generate_hook_registrations(hooks: &[HookInfo]) -> Vec<TokenStream2> {
    hooks
        .iter()
        .map(|hook| {
            let fn_ident = &hook.fn_ident;

            let hook_name = &hook.name;
            match hook.hook_type {
                HookType::BeforeAll => quote! {
                    suite.hooks = suite.hooks.before_all(#hook_name, |_ctx| {
                        #fn_ident();
                        Ok(())
                    });
                },
                HookType::AfterAll => quote! {
                    suite.hooks = suite.hooks.after_all(#hook_name, |_ctx| {
                        #fn_ident();
                        Ok(())
                    });
                },
                HookType::BeforeEach => quote! {
                    suite.hooks = suite.hooks.before_each(#hook_name, |_ctx| {
                        #fn_ident();
                        Ok(())
                    });
                },
                HookType::AfterEach => quote! {
                    suite.hooks = suite.hooks.after_each(#hook_name, |_ctx| {
                        #fn_ident();
                        Ok(())
                    });
                },
            }
        })
        .collect()
}

fn parse_sheila_attribute(attr: &Attribute) -> Option<SheilaAttribute> {
    let path = &attr.path();
    let segments: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();

    let is_sheila = segments.len() == 2 && segments[0] == "sheila";
    let attr_name = if is_sheila {
        &segments[1]
    } else if segments.len() == 1 {
        &segments[0]
    } else {
        return None;
    };

    match attr_name.as_str() {
        "test" => {
            let tags = parse_test_attribute(attr);
            Some(SheilaAttribute::Test { tags })
        }
        "fixture" => {
            let (scope, depends_on) = parse_fixture_attribute(attr);
            Some(SheilaAttribute::Fixture { scope, depends_on })
        }
        "before_all" => Some(SheilaAttribute::Hook(HookType::BeforeAll)),
        "after_all" => Some(SheilaAttribute::Hook(HookType::AfterAll)),
        "before_each" => Some(SheilaAttribute::Hook(HookType::BeforeEach)),
        "after_each" => Some(SheilaAttribute::Hook(HookType::AfterEach)),
        _ => None,
    }
}

fn parse_test_attribute(attr: &Attribute) -> Vec<String> {
    let mut tags = Vec::new();

    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("tags") {
            let value = meta.value()?;
            let array_str: syn::LitStr = value.parse()?;
            tags = parse_string_array(&array_str.value());
        }
        Ok(())
    });

    tags
}

fn parse_fixture_attribute(attr: &Attribute) -> (String, Vec<String>) {
    let mut scope = "test".to_string();
    let mut depends_on = Vec::new();

    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("scope") {
            let value = meta.value()?;
            let lit: syn::LitStr = value.parse()?;
            scope = lit.value();
        } else if meta.path.is_ident("depends_on") {
            let value = meta.value()?;
            let array_str: syn::LitStr = value.parse()?;
            depends_on = parse_string_array(&array_str.value());
        }
        Ok(())
    });

    (scope, depends_on)
}

fn parse_attribute_args(args: TokenStream) -> Vec<String> {
    if args.is_empty() {
        return vec![];
    }

    let args_str = args.to_string();
    args_str
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .collect()
}

fn parse_string_array(s: &str) -> Vec<String> {
    s.trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
