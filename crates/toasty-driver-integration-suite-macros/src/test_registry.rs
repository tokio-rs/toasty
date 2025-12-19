use proc_macro::TokenStream;
use quote::quote;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{parse_macro_input, visit::Visit, File, ItemFn, LitStr};

use crate::parse::DriverTest;

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
    let content =
        fs::read_to_string(path).expect(&format!("Failed to read file: {}", path.display()));

    let file =
        syn::parse_file(&content).expect(&format!("Failed to parse file: {}", path.display()));

    let mut visitor = TestVisitor { tests: Vec::new() };

    visitor.visit_file(&file);

    visitor.tests
}

struct TestVisitor {
    tests: Vec<DriverTest>,
}

impl<'ast> Visit<'ast> for TestVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        // Check if this function has the #[driver_test] attribute
        let has_driver_test = node
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("driver_test"));

        if has_driver_test {
            // Use the shared parsing logic
            let driver_test = DriverTest::from_item_fn(node.clone());
            self.tests.push(driver_test);
        }

        // Continue visiting
        syn::visit::visit_item_fn(self, node);
    }
}

fn generate_macro(structure: TestStructure) -> TokenStream {
    let mut module_tokens = Vec::new();

    for (module_name, tests) in structure.modules {
        let module_ident = syn::Ident::new(&module_name, proc_macro2::Span::call_site());

        let mut test_tokens = Vec::new();
        for driver_test in tests {
            let test_ident = &driver_test.name;
            let variant_idents: Vec<_> = driver_test.kinds.iter().map(|k| k.ident()).collect();

            test_tokens.push(quote! {
                #test_ident { #(#variant_idents)* }
            });
        }

        module_tokens.push(quote! {
            #module_ident {
                #(#test_tokens)*
            }
        });
    }

    let expanded = quote! {
        #[macro_export]
        macro_rules! generate_driver_tests {
            ($driver:expr) => {
                $crate::generate_driver_tests_impl! {
                    $driver,
                    #(#module_tokens)*
                }
            };
        }
    };

    expanded.into()
}
