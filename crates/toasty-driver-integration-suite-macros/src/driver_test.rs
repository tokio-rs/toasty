use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input, visit_mut::VisitMut};

use crate::id_rewriter::IdRewriter;
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
                generate_variant(
                    &driver_test.input,
                    expansion,
                    &driver_test.requires,
                    driver_test.attr.scenario.as_ref(),
                    true,
                )
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
            driver_test.attr.scenario.as_ref(),
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
    scenario: Option<&syn::Path>,
    use_expansion_name: bool,
) -> ItemFn {
    let mut variant = input.clone();

    // Update function name based on whether we're inside a module
    if use_expansion_name && let Some(expansion_ident) = expansion.to_ident() {
        // Inside a module: use just the expansion name (e.g., "id_uuid")
        variant.sig.ident = expansion_ident;
    }
    // Otherwise keep the original function name

    // Don't add #[tokio::test] or #[test] attributes - the test registry in the consuming
    // crate will add them. If we add test attributes here, the functions become test-only
    // items that aren't accessible as regular library functions.

    // Process driver_test_cfg attributes
    process_driver_test_cfg_attrs(&mut variant, expansion);

    // Rewrite driver_test_cfg! macro calls to boolean literals
    rewrite_driver_test_cfg_macros(&mut variant, expansion);

    // Rewrite ID types if expansion has an ID variant
    if let (Some(id_ident), Some(id_variant)) = (&expansion.id_ident, &expansion.id_variant) {
        let target_type = match id_variant {
            crate::parse::KindVariant::IdU64 => syn::parse_quote!(u64),
            crate::parse::KindVariant::IdUuid => syn::parse_quote!(uuid::Uuid),
        };
        let mut rewriter = IdRewriter::new(id_ident, target_type);
        rewriter.visit_item_fn_mut(&mut variant);
    }

    // Inject scenario use-import if specified
    if let Some(scenario_path) = scenario {
        let use_stmt: syn::Stmt = if let Some(ref id_variant) = expansion.id_variant {
            let variant_ident = syn::Ident::new(id_variant.name(), proc_macro2::Span::call_site());
            syn::parse_quote! {
                use #scenario_path::#variant_ident::*;
            }
        } else {
            syn::parse_quote! {
                use #scenario_path::*;
            }
        };
        variant.block.stmts.insert(0, use_stmt);
    }

    // Add capability checks at the beginning of the function if there are requires
    if let Some(requires_expr) = requires {
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

/// Add a runtime check that the driver satisfies the test's `requires(...)`.
///
/// The expansion already knows its own matrix dimensions and ID variant, and
/// the per-driver list in `generate_driver_tests!` already filtered expansions
/// the driver cannot run. What is left is the capability reads, which the
/// generated test asserts against the driver's live [`Capability`].
///
/// The whole expression is asserted as one condition, not each capability on
/// its own: under `or(...)` a single capability is not individually required,
/// and asserting it would fail a driver that satisfies the other branch.
///
/// [`Capability`]: toasty_core::driver::Capability
fn add_capability_checks_from_expr(
    func: &mut ItemFn,
    requires_expr: &BoolExpr,
    expansion: &Expansion,
) {
    use syn::parse_quote;

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

    let capability = quote! { #test_param.capability() };

    let Some(condition) = capability_condition(requires_expr, expansion, &capability) else {
        // The expansion's own idents settle the expression; no capability to
        // read at runtime.
        return;
    };

    let rendered = render_bool_expr(requires_expr);
    let check: syn::Stmt = parse_quote! {
        assert!(
            #condition,
            "driver does not satisfy requires({})",
            #rendered
        );
    };

    // Prepend the check to the function body
    let original_block = &func.block;
    func.block = parse_quote! {
        {
            #check
            #original_block
        }
    };
}

/// A `requires(...)` sub-expression after folding the expansion's own idents.
enum Folded {
    /// Settled by the expansion alone — no capability read involved.
    Known(bool),

    /// A condition to evaluate against the driver's live capabilities.
    Dynamic(proc_macro2::TokenStream),
}

/// Build the runtime condition for a `requires(...)` expression, folding away
/// everything the expansion already settles.
///
/// Returns `None` when folding leaves no capability read — the expression is
/// then either satisfied by construction or excluded at expansion time.
fn capability_condition(
    expr: &BoolExpr,
    expansion: &Expansion,
    capability: &proc_macro2::TokenStream,
) -> Option<proc_macro2::TokenStream> {
    match fold(expr, expansion, capability) {
        Folded::Dynamic(condition) => Some(condition),
        Folded::Known(_) => None,
    }
}

fn fold(expr: &BoolExpr, expansion: &Expansion, capability: &proc_macro2::TokenStream) -> Folded {
    match expr {
        BoolExpr::Ident(name) => {
            // Matrix dimensions and ID variants are fixed for this expansion,
            // and test flags describe the suite rather than the database, so
            // none of them has a `Capability` field to read.
            if expansion.is_ident_true(name) || crate::parse::is_test_flag(name) {
                return Folded::Known(true);
            }

            if expansion.is_ident_explicitly_false(name) {
                return Folded::Known(false);
            }

            let cap = syn::Ident::new(name, proc_macro2::Span::call_site());
            Folded::Dynamic(crate::parse::read_capability(capability.clone(), &cap))
        }
        BoolExpr::Or(exprs) => {
            let mut operands = Vec::new();

            for expr in exprs {
                match fold(expr, expansion, capability) {
                    // One settled branch carries the whole expression.
                    Folded::Known(true) => return Folded::Known(true),
                    Folded::Known(false) => {}
                    Folded::Dynamic(operand) => operands.push(operand),
                }
            }

            join(operands, quote! { || }, false)
        }
        BoolExpr::And(exprs) => {
            let mut operands = Vec::new();

            for expr in exprs {
                match fold(expr, expansion, capability) {
                    Folded::Known(false) => return Folded::Known(false),
                    Folded::Known(true) => {}
                    Folded::Dynamic(operand) => operands.push(operand),
                }
            }

            join(operands, quote! { && }, true)
        }
        BoolExpr::Not(inner) => match fold(inner, expansion, capability) {
            Folded::Known(value) => Folded::Known(!value),
            Folded::Dynamic(inner) => Folded::Dynamic(quote! { !(#inner) }),
        },
    }
}

/// Join operands with `op`, parenthesizing so nesting keeps its meaning. With
/// every operand folded away, the expression settles to `identity`.
fn join(
    operands: Vec<proc_macro2::TokenStream>,
    op: proc_macro2::TokenStream,
    identity: bool,
) -> Folded {
    let mut operands = operands.into_iter();

    let Some(first) = operands.next() else {
        return Folded::Known(identity);
    };

    Folded::Dynamic(operands.fold(quote! { (#first) }, |acc, operand| {
        quote! { #acc #op (#operand) }
    }))
}

/// Render a `requires(...)` expression back to source form for error messages.
fn render_bool_expr(expr: &BoolExpr) -> String {
    match expr {
        BoolExpr::Ident(name) => name.clone(),
        BoolExpr::Or(exprs) => format!("or({})", render_list(exprs)),
        BoolExpr::And(exprs) => format!("and({})", render_list(exprs)),
        BoolExpr::Not(inner) => format!("not({})", render_bool_expr(inner)),
    }
}

fn render_list(exprs: &[BoolExpr]) -> String {
    exprs
        .iter()
        .map(render_bool_expr)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Rewrite driver_test_cfg! macro calls to boolean literals based on the expansion
fn rewrite_driver_test_cfg_macros(func: &mut ItemFn, expansion: &Expansion) {
    struct MacroRewriter<'a> {
        expansion: &'a Expansion,
    }

    impl<'a> VisitMut for MacroRewriter<'a> {
        fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
            // Check if this is a macro call to driver_test_cfg!
            if let syn::Expr::Macro(expr_macro) = expr
                && expr_macro.mac.path.is_ident("driver_test_cfg")
            {
                // Parse the boolean expression from the macro tokens
                let tokens = expr_macro.mac.tokens.clone();
                if let Ok(bool_expr) = syn::parse::Parser::parse2(
                    |input: syn::parse::ParseStream| BoolExpr::parse(input),
                    tokens,
                ) {
                    // Evaluate the expression for this expansion
                    let result = self.expansion.evaluate_predicate(&bool_expr, &|_ident| {
                        // Database capabilities are unknown at compile time
                        // Return Unknown, which should not affect evaluation of
                        // compile-time known values (matrix and ID variants)
                        crate::parse::ThreeValuedBool::Unknown
                    });

                    // Convert to boolean literal
                    let bool_value = match result {
                        crate::parse::ThreeValuedBool::True => true,
                        crate::parse::ThreeValuedBool::False => false,
                        crate::parse::ThreeValuedBool::Unknown => {
                            // Unknown means the expression references database capabilities,
                            // which can only be checked at runtime, not compile time.
                            // This is a compile error - driver_test_cfg! should only be used
                            // for compile-time known values (ID variants, matrix dimensions).
                            panic!(
                                "driver_test_cfg! can only be used with compile-time known values \
                                     (id_u64, id_uuid, matrix dimensions). Database capabilities must \
                                     be checked at runtime using test.capability()"
                            );
                        }
                    };

                    // Replace the macro call with a boolean literal
                    *expr = syn::parse_quote!(#bool_value);
                    return;
                }
            }

            // Continue visiting nested expressions
            syn::visit_mut::visit_expr_mut(self, expr);
        }
    }

    let mut rewriter = MacroRewriter { expansion };
    rewriter.visit_item_fn_mut(func);
}
