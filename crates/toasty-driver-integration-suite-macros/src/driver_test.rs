use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, visit_mut::VisitMut, ItemFn, Type, TypePath};

use crate::parse::{BoolExpr, DriverTest, DriverTestAttr, Expansion, ExpansionList};

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let attr = parse_macro_input!(attr as DriverTestAttr);

    // Parse the driver test using shared logic
    let driver_test = DriverTest::from_item_fn(input, attr);

    let mod_name = &driver_test.name;
    let vis = &driver_test.input.vis;

    assert!(
        !driver_test.expansions.is_empty(),
        "driver_test={driver_test:#?}"
    );

    // Check if we need to wrap variants in a module
    if driver_test.expansions.needs_module_wrapper() {
        // Generate variants using expansion logic
        // When inside a module, use the expansion name as the function name
        let variant_fns: Vec<_> = driver_test
            .expansions
            .iter()
            .map(|expansion| {
                generate_variant(&driver_test.input, expansion, &driver_test.requires, true)
            })
            .collect();

        quote! {
            #vis mod #mod_name {
                use super::*;

                #(#variant_fns)*
            }
        }
        .into()
    } else {
        // Single expansion with no name - return the function directly
        let variant = generate_variant(
            &driver_test.input,
            &driver_test.expansions[0],
            &driver_test.requires,
            false, // Don't use expansion name as function name
        );
        quote! {
            #variant
        }
        .into()
    }
}

/// Generate a test variant with ID rewritten to the target type
fn generate_variant(
    input: &ItemFn,
    expansion: &Expansion,
    requires: &Option<BoolExpr>,
    use_expansion_name: bool,
) -> ItemFn {
    let mut variant = input.clone();

    // Update function name based on whether we're inside a module
    if use_expansion_name {
        if let Some(expansion_ident) = expansion.to_ident() {
            // Inside a module: use just the expansion name (e.g., "id_uuid")
            variant.sig.ident = expansion_ident;
        }
    }
    // Otherwise keep the original function name

    // Don't add #[tokio::test] or #[test] attributes - the test registry in the consuming
    // crate will add them. If we add test attributes here, the functions become test-only
    // items that aren't accessible as regular library functions.

    // Process driver_test_cfg attributes
    process_driver_test_cfg_attrs(&mut variant, expansion);

    // Rewrite ID types if expansion has an ID variant
    if let (Some(ref id_ident), Some(ref id_variant)) = (&expansion.id_ident, &expansion.id_variant)
    {
        let target_type = match id_variant {
            crate::parse::KindVariant::IdU64 => syn::parse_quote!(u64),
            crate::parse::KindVariant::IdUuid => syn::parse_quote!(uuid::Uuid),
        };
        let mut rewriter = IdRewriter::new(id_ident, target_type);
        rewriter.visit_item_fn_mut(&mut variant);
    }

    // Add capability checks at the beginning of the function if there are requires
    if let Some(ref requires_expr) = requires {
        add_capability_checks_from_expr(&mut variant, requires_expr, expansion);
    }

    variant
}

/// Process driver_test_cfg attributes, either keeping or removing them based on the expansion
fn process_driver_test_cfg_attrs(func: &mut ItemFn, expansion: &Expansion) {
    // Process attributes in the function body (on items like struct definitions)
    struct AttrProcessor<'a> {
        expansion: &'a Expansion,
    }

    impl<'a> VisitMut for AttrProcessor<'a> {
        fn visit_item_struct_mut(&mut self, node: &mut syn::ItemStruct) {
            process_attrs(&mut node.attrs, self.expansion);
            syn::visit_mut::visit_item_struct_mut(self, node);
        }

        fn visit_field_mut(&mut self, node: &mut syn::Field) {
            process_attrs(&mut node.attrs, self.expansion);
            syn::visit_mut::visit_field_mut(self, node);
        }
    }

    let mut processor = AttrProcessor { expansion };
    processor.visit_item_fn_mut(func);
}

/// Process attributes for a single item
fn process_attrs(attrs: &mut Vec<syn::Attribute>, expansion: &Expansion) {
    let mut new_attrs = Vec::new();

    for attr in attrs.drain(..) {
        if attr.path().is_ident("driver_test_cfg") {
            // Parse driver_test_cfg(condition, attr)
            // We expect: #[driver_test_cfg(condition, attr(...))]
            if let syn::Meta::List(ref meta_list) = attr.meta {
                let tokens = &meta_list.tokens;

                // Try to parse manually: condition_ident, remaining_tokens
                let token_string = tokens.to_string();
                if let Some(comma_pos) = token_string.find(',') {
                    let condition_ident = token_string[..comma_pos].trim();
                    let remaining = token_string[comma_pos + 1..].trim();

                    // Check if the condition is true for this expansion
                    if expansion.is_ident_true(condition_ident) {
                        // Parse the remaining part as an attribute (without the #[...] wrapper)
                        // We need to add the #[...] wrapper to parse it correctly
                        let attr_string = format!("#[{}]", remaining);
                        if let Ok(parsed_attrs) =
                            syn::parse::Parser::parse_str(syn::Attribute::parse_outer, &attr_string)
                        {
                            new_attrs.extend(parsed_attrs);
                        }
                    }
                }
            }
        } else {
            // Keep non-driver_test_cfg attributes
            new_attrs.push(attr);
        }
    }

    *attrs = new_attrs;
}

/// Add runtime capability checks for database capabilities in the requires expression
fn add_capability_checks_from_expr(
    func: &mut ItemFn,
    requires_expr: &BoolExpr,
    expansion: &Expansion,
) {
    use syn::{parse_quote, Ident, Stmt};

    // Extract database capability identifiers from the expression
    let db_capabilities = extract_db_capabilities(requires_expr, expansion);

    if db_capabilities.is_empty() {
        return;
    }

    // Get the test parameter name (first parameter of the function)
    let test_param = func
        .sig
        .inputs
        .first()
        .and_then(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    Some(&pat_ident.ident)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("Test function must have at least one parameter");

    // Generate capability check statements
    let capability_checks: Vec<Stmt> = db_capabilities
        .iter()
        .map(|(cap_name, is_negated)| {
            let cap_ident = Ident::new(cap_name, proc_macro2::Span::call_site());
            if *is_negated {
                // For negated capabilities, check that the capability is NOT present
                parse_quote! {
                    assert!(
                        !#test_param.capability().#cap_ident,
                        "Driver should not support capability: {}",
                        stringify!(#cap_ident)
                    );
                }
            } else {
                // For regular capabilities, check that it IS present
                parse_quote! {
                    assert!(
                        #test_param.capability().#cap_ident,
                        "Driver does not support required capability: {}",
                        stringify!(#cap_ident)
                    );
                }
            }
        })
        .collect();

    // Prepend the checks to the function body
    let original_block = &func.block;
    func.block = parse_quote! {
        {
            #(#capability_checks)*
            #original_block
        }
    };
}

/// Extract database capability requirements from a boolean expression
fn extract_db_capabilities(expr: &BoolExpr, expansion: &Expansion) -> Vec<(String, bool)> {
    fn extract_recursive(
        expr: &BoolExpr,
        expansion: &Expansion,
        negated: bool,
        result: &mut Vec<(String, bool)>,
    ) {
        match expr {
            BoolExpr::Ident(name) => {
                // Check if this is a matrix identifier
                if expansion.matrix_values.contains_key(name) {
                    return; // Skip matrix identifiers
                }

                // Check if this is an ID variant identifier
                if name == "id_u64" || name == "id_uuid" {
                    return; // Skip ID variant identifiers
                }

                // This must be a database capability
                result.push((name.clone(), negated));
            }
            BoolExpr::Or(exprs) | BoolExpr::And(exprs) => {
                for e in exprs {
                    extract_recursive(e, expansion, negated, result);
                }
            }
            BoolExpr::Not(inner) => {
                extract_recursive(inner, expansion, !negated, result);
            }
        }
    }

    let mut result = Vec::new();
    extract_recursive(expr, expansion, false, &mut result);
    result
}

/// Visitor that rewrites type references to a configurable target type
struct IdRewriter {
    /// The identifier to replace (e.g., "ID")
    ident: String,
    /// The target type to replace with
    target_type: Type,
}

impl IdRewriter {
    fn new(ident: &str, target_type: Type) -> Self {
        Self {
            ident: ident.to_string(),
            target_type,
        }
    }
}

impl VisitMut for IdRewriter {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(TypePath { qself: None, path }) = ty {
            // Check if this matches the identifier we're looking for
            if path.segments.len() == 1 && path.segments[0].ident == self.ident {
                // Replace with target type
                *ty = self.target_type.clone();
                return;
            }
        }

        // Continue visiting nested types
        syn::visit_mut::visit_type_mut(self, ty);
    }
}
