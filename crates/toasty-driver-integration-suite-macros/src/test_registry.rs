use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{parse_macro_input, visit::Visit, Ident, ItemFn, LitStr};

use crate::parse::{DriverTest, DriverTestAttr};

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
}

fn scan_test_directory(dir: &Path) -> TestStructure {
    let mut structure = TestStructure {
        modules: BTreeMap::new(),
    };

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

            if !tests.is_empty() {
                structure.modules.insert(module_name, tests);
            }
        }
    }

    structure
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
            // Parse the attribute arguments
            let parsed_attr = if attr.meta.require_path_only().is_ok() {
                // Empty attribute: #[driver_test]
                DriverTestAttr {
                    id_ident: None,
                    requires: Vec::new(),
                }
            } else {
                // Parse attribute arguments: #[driver_test(id(ID), requires(...))]
                attr.parse_args::<DriverTestAttr>()
                    .unwrap_or_else(|e| panic!("Failed to parse #[driver_test] attribute: {}", e))
            };

            // Use the shared parsing logic
            let driver_test = DriverTest::from_item_fn(node.clone(), parsed_attr);
            self.tests.push(driver_test);
        }

        // Continue visiting
        syn::visit::visit_item_fn(self, node);
    }
}

fn generate_macro(structure: TestStructure) -> TokenStream {
    // Generate the module structure with all tests inlined
    let modules: Vec<TokenStream2> = structure
        .modules
        .iter()
        .map(|(module_name, tests)| {
            let module_ident = Ident::new(module_name, proc_macro2::Span::call_site());

            let test_modules: Vec<TokenStream2> = tests
                .iter()
                .map(|test| {
                    let test_ident = &test.name;

                    // If test has no kinds, it's not ID-parameterized, so generate a single test
                    // Otherwise, generate a module with variants
                    if test.kinds.is_empty() {
                        quote! {
                            #[test]
                            fn #test_ident() {
                                let mut test = $crate::Test::new(
                                    ::std::sync::Arc::new($driver_expr)
                                );

                                test.run(async move |t| {
                                    $crate::tests::#module_ident::#test_ident(t).await;
                                });
                            }
                        }
                    } else {
                        // Generate requires list as comma-separated identifiers
                        let requires_list: Vec<_> = test
                            .requires
                            .iter()
                            .map(|r| Ident::new(r, proc_macro2::Span::call_site()))
                            .collect();

                        quote! {
                            mod #test_ident {
                                use super::*;

                                $crate::generate_driver_test_variants!(
                                    $crate,
                                    #module_ident::#test_ident,
                                    $driver_expr,
                                    requires: [#(#requires_list),*]
                                        $(, $($t)* )?
                                );
                            }
                        }
                    }
                })
                .collect();

            quote! {
                mod #module_ident {
                    use super::*;

                    #(#test_modules)*
                }
            }
        })
        .collect();

    let expanded = quote! {
        #[macro_export]
        macro_rules! generate_driver_tests {
            ($driver_expr:expr $(, $($t:tt)* )?) => {
                #(#modules)*
            };
        }
    };

    expanded.into()
}
