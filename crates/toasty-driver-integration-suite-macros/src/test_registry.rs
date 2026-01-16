use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{parse_macro_input, visit::Visit, Ident, ItemFn, LitStr};

use crate::parse::{BoolExpr, DriverTest, DriverTestAttr};

pub fn expand(input: TokenStream) -> TokenStream {
    // Parse the relative path to the tests directory (relative to CARGO_MANIFEST_DIR)
    let relative_path = parse_macro_input!(input as LitStr);

    // Get the manifest directory of the crate invoking this macro
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    // Construct the full path
    let tests_dir = PathBuf::from(manifest_dir).join(relative_path.value());

    if !tests_dir.exists() {
        panic!("Tests directory not found at: {}", tests_dir.display());
    }

    // Scan for test files and extract test names
    let test_structure = scan_test_directory(&tests_dir);

    // Generate the macro invocation
    generate_macro(test_structure)
}

/// Represents the hierarchical structure of tests
#[derive(Debug)]
struct TestStructure {
    /// Map of module name -> tests in that module
    modules: BTreeMap<String, Vec<DriverTest>>,
    /// All unique capabilities required across all tests
    requires: Vec<Ident>,
}

fn scan_test_directory(dir: &Path) -> TestStructure {
    let mut structure = TestStructure {
        modules: BTreeMap::new(),
        requires: Vec::new(),
    };

    let mut all_requires = std::collections::HashSet::new();

    // Read all .rs files in the directory
    for entry in fs::read_dir(dir).expect("Failed to read tests directory") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let module_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("Invalid file name")
                .to_string();

            // Skip mod.rs
            if module_name == "mod" {
                continue;
            }

            // Parse the file and extract test functions
            let tests = extract_tests_from_file(&path);

            // Collect all unique requires from these tests
            for test in &tests {
                if let Some(ref req) = test.requires {
                    all_requires.insert(req.clone());
                }
            }

            if !tests.is_empty() {
                structure.modules.insert(module_name, tests);
            }
        }
    }

    // Extract all capability identifiers from the BoolExpr requirements
    let mut capability_names = std::collections::HashSet::new();
    for req_expr in &all_requires {
        extract_capability_names(req_expr, &mut capability_names);
    }

    // Always include auto_increment as it's implicitly required by ID expansion
    capability_names.insert("auto_increment".to_string());

    // Convert HashSet to sorted Vec of Idents
    let mut requires_vec: Vec<_> = capability_names.into_iter().collect();
    requires_vec.sort();
    structure.requires = requires_vec
        .into_iter()
        .map(|name| Ident::new(&name, proc_macro2::Span::call_site()))
        .collect();

    structure
}

/// Extract all capability names from a BoolExpr (ignoring matrix identifiers)
fn extract_capability_names(expr: &BoolExpr, result: &mut std::collections::HashSet<String>) {
    match expr {
        BoolExpr::Ident(name) => {
            // Skip common matrix identifiers
            if name != "single" && name != "composite" && name != "id_u64" && name != "id_uuid" {
                result.insert(name.clone());
            }
        }
        BoolExpr::Or(exprs) | BoolExpr::And(exprs) => {
            for e in exprs {
                extract_capability_names(e, result);
            }
        }
        BoolExpr::Not(inner) => {
            extract_capability_names(inner, result);
        }
    }
}

fn extract_tests_from_file(path: &Path) -> Vec<DriverTest> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("Failed to read file: {}", path.display()));

    let file = syn::parse_file(&content)
        .unwrap_or_else(|_| panic!("Failed to parse file: {}", path.display()));

    let mut visitor = TestVisitor { tests: Vec::new() };

    visitor.visit_file(&file);

    visitor.tests
}

struct TestVisitor {
    tests: Vec<DriverTest>,
}

impl<'ast> Visit<'ast> for TestVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        // Check if this function has the #[driver_test] attribute and extract it
        let driver_test_attr = node
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("driver_test"));

        if let Some(attr) = driver_test_attr {
            // Parse the attribute using from_attribute
            let parsed_attr = DriverTestAttr::from_attribute(attr)
                .unwrap_or_else(|e| panic!("Failed to parse #[driver_test] attribute: {}", e));

            // Use the shared parsing logic
            let driver_test = DriverTest::from_item_fn(node.clone(), parsed_attr);
            self.tests.push(driver_test);
        }

        // Continue visiting
        syn::visit::visit_item_fn(self, node);
    }
}

fn generate_macro(structure: TestStructure) -> TokenStream {
    // Generate the module structure with test invocations
    let modules: Vec<TokenStream2> = structure
        .modules
        .iter()
        .map(|(module_name, tests)| {
            let module_ident = Ident::new(module_name, proc_macro2::Span::call_site());

            let test_invocations: Vec<TokenStream2> = tests
                .iter()
                .map(|test| {
                    let test_ident = &test.name;
                    let attr = &test.attr.ast;
                    let extra_attrs = &test.attrs;

                    // Generate the full module path with driver_test attribute and extra attributes
                    // Pass attrs as token trees to avoid macro escaping issues
                    quote! {
                        $crate::generate_driver_test_variants!($driver_expr, $crate, #module_ident::#test_ident, #attr, attrs[#(#extra_attrs)*], capability( $( $($t)* )? ));
                    }
                })
                .collect();

            quote! {
                mod #module_ident {
                    use super::*;

                    #(#test_invocations)*
                }
            }
        })
        .collect();

    // Generate capability validation function
    let capability_checks: Vec<_> = structure
        .requires
        .iter()
        .map(|cap| {
            quote! {
                let _ = cap.#cap;
            }
        })
        .collect();

    let capability_validation = if !structure.requires.is_empty() {
        quote! {
            // Validate driver capabilities at compile time
            const _: () = {
                async fn __validate_capabilities(cap: &toasty_core::driver::Capability) {
                    #(#capability_checks)*
                }
            };
        }
    } else {
        quote! {}
    };

    // Generate runtime capability validation test
    let capability_runtime_test = generate_capability_runtime_test(&structure);

    let expanded = quote! {
        #[macro_export]
        macro_rules! generate_driver_tests {
            ($driver_expr:expr $(, $($t:tt)* )?) => {
                #capability_validation

                #capability_runtime_test

                #(#modules)*
            };
        }
    };

    expanded.into()
}

/// Generate a runtime test that validates driver capabilities match the specified requirements
fn generate_capability_runtime_test(structure: &TestStructure) -> TokenStream2 {
    if structure.requires.is_empty() {
        return quote! {};
    }

    let requires_list = &structure.requires;

    quote! {
        #[test]
        fn validate_driver_capabilities() {
            let mut test = $crate::Test::new(
                ::std::sync::Arc::new($driver_expr)
            );

            test.run(async move |t| {
                let capability = t.capability();

                // Parse capability flags from macro arguments
                let mut expected_capabilities = ::std::collections::HashMap::new();

                // Default all capabilities to true
                #(
                    expected_capabilities.insert(stringify!(#requires_list).to_string(), true);
                )*

                // Override with user-specified values
                $(
                    __parse_capability_flags(&mut expected_capabilities, stringify!($($t)*));
                )?

                // Validate each capability matches expected value
                #(
                    let expected = expected_capabilities.get(stringify!(#requires_list)).copied().unwrap_or(true);
                    assert_eq!(
                        capability.#requires_list,
                        expected,
                        "Capability mismatch for {}: expected {}, got {}",
                        stringify!(#requires_list),
                        expected,
                        capability.#requires_list
                    );
                )*
            });
        }

        #[allow(dead_code)]
        fn __parse_capability_flags(map: &mut ::std::collections::HashMap<String, bool>, input: &str) {
            // Parse "cap1: false, cap2: true" format
            for part in input.split(',') {
                let part = part.trim();
                if let Some((key, value)) = part.split_once(':') {
                    let key = key.trim();
                    let value = value.trim();

                    let bool_value = match value {
                        "true" => true,
                        "false" => false,
                        _ => continue,
                    };

                    // Map short names to full capability field names
                    match key {
                        "bigdecimal" => {
                            assert!(map.contains_key("bigdecimal_implemented"), "not a valid capability: {key:#?}");
                            map.insert("bigdecimal_implemented".to_string(), bool_value);
                        }
                        "decimal" => {
                            assert!(map.contains_key("decimal_arbitrary_precision"), "not a valid capability: {key:#?}");
                            assert!(map.contains_key("native_decimal"), "not a valid capability: {key:#?}");
                            map.insert("decimal_arbitrary_precision".to_string(), bool_value);
                            map.insert("native_decimal".to_string(), bool_value);
                        }
                        other => {
                            map.insert(other.to_string(), bool_value);
                        }
                    }
                }
            }
        }
    }
}
