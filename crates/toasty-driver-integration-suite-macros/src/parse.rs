use syn::{parse::Parse, punctuated::Punctuated, token::Comma, Ident, ItemFn};

/// Helper trait for working with collections of expansions
pub trait ExpansionList {
    /// Check if these expansions require a module wrapper
    /// Returns true if there are multiple expansions or a single expansion with a non-empty name
    fn needs_module_wrapper(&self) -> bool;
}

impl ExpansionList for [Expansion] {
    fn needs_module_wrapper(&self) -> bool {
        self.len() > 1 || (self.len() == 1 && self[0].has_expansion())
    }
}

/// Parsed representation of a `#[driver_test]` function
#[derive(Debug, Clone)]
pub struct DriverTest {
    /// The original function (before transformation)
    pub input: ItemFn,

    /// Test function name
    pub name: syn::Ident,

    /// List of test expansions to generate
    pub expansions: Vec<Expansion>,

    /// Required capabilities for this test
    pub requires: Option<BoolExpr>,

    /// Non-driver_test attributes (e.g., #[should_panic], #[ignore])
    pub attrs: Vec<syn::Attribute>,

    /// The parsed attribute
    pub attr: DriverTestAttr,
}

/// Three-valued boolean logic for predicate evaluation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreeValuedBool {
    True,
    False,
    Unknown,
}

impl ThreeValuedBool {
    /// Logical OR for three-valued logic
    pub fn or(self, other: Self) -> Self {
        match (self, other) {
            (ThreeValuedBool::True, _) | (_, ThreeValuedBool::True) => ThreeValuedBool::True,
            (ThreeValuedBool::False, ThreeValuedBool::False) => ThreeValuedBool::False,
            _ => ThreeValuedBool::Unknown,
        }
    }

    /// Logical AND for three-valued logic
    pub fn and(self, other: Self) -> Self {
        match (self, other) {
            (ThreeValuedBool::False, _) | (_, ThreeValuedBool::False) => ThreeValuedBool::False,
            (ThreeValuedBool::True, ThreeValuedBool::True) => ThreeValuedBool::True,
            _ => ThreeValuedBool::Unknown,
        }
    }

    /// Logical NOT for three-valued logic
    pub fn not(self) -> Self {
        match self {
            ThreeValuedBool::True => ThreeValuedBool::False,
            ThreeValuedBool::False => ThreeValuedBool::True,
            ThreeValuedBool::Unknown => ThreeValuedBool::Unknown,
        }
    }
}

/// A single test expansion with specific matrix values
#[derive(Debug, Clone)]
pub struct Expansion {
    /// The ID variant (if id() was specified)
    pub id_variant: Option<KindVariant>,

    /// The identifier to replace for ID (e.g., "ID")
    pub id_ident: Option<String>,

    /// Matrix dimension values (e.g., {"single": true, "composite": false})
    pub matrix_values: std::collections::HashMap<String, bool>,

    /// The predicate to evaluate for this expansion
    pub predicate: Option<BoolExpr>,
}

impl Expansion {
    /// Generate the test function name for this expansion
    pub fn name(&self) -> String {
        let mut parts = Vec::new();

        // Add matrix dimensions that are true (in sorted order for consistency)
        let mut matrix_keys: Vec<_> = self.matrix_values.keys().collect();
        matrix_keys.sort();
        for key in matrix_keys {
            if self.matrix_values[key] {
                parts.push(key.clone());
            }
        }

        // Add ID variant
        if let Some(ref variant) = self.id_variant {
            parts.push(variant.name().to_string());
        }

        parts.join("_")
    }

    /// Check if an identifier is true for this expansion
    pub fn is_ident_true(&self, ident: &str) -> bool {
        // Check matrix values
        if let Some(&value) = self.matrix_values.get(ident) {
            return value;
        }

        // Check ID variant identifiers
        if let Some(ref variant) = self.id_variant {
            match variant {
                KindVariant::IdU64 if ident == "id_u64" => return true,
                KindVariant::IdUuid if ident == "id_uuid" => return true,
                _ => {}
            }
        }

        false
    }

    /// Determine if this expansion should be included based on the predicate
    ///
    /// Returns true if:
    /// - No predicate exists
    /// - Predicate evaluates to True
    /// - Predicate evaluates to Unknown (conservative inclusion)
    ///
    /// Returns false only if predicate evaluates to False
    pub fn should_include<F>(&self, get_value: F) -> bool
    where
        F: Fn(&str) -> ThreeValuedBool,
    {
        let Some(ref predicate) = self.predicate else {
            return true; // No predicate means always include
        };

        match self.evaluate_predicate(predicate, &get_value) {
            ThreeValuedBool::False => false,
            ThreeValuedBool::True | ThreeValuedBool::Unknown => true,
        }
    }

    /// Evaluate a boolean expression using three-valued logic
    fn evaluate_predicate<F>(&self, expr: &BoolExpr, get_value: &F) -> ThreeValuedBool
    where
        F: Fn(&str) -> ThreeValuedBool,
    {
        match expr {
            BoolExpr::Ident(name) => {
                // Check if this is a matrix dimension or ID variant identifier
                if self.is_ident_true(name) {
                    return ThreeValuedBool::True;
                }
                if self.is_ident_explicitly_false(name) {
                    return ThreeValuedBool::False;
                }
                // Otherwise check via the get_value closure (for capabilities)
                get_value(name)
            }
            BoolExpr::Or(exprs) => exprs
                .iter()
                .map(|e| self.evaluate_predicate(e, get_value))
                .fold(ThreeValuedBool::False, |acc, val| acc.or(val)),
            BoolExpr::And(exprs) => exprs
                .iter()
                .map(|e| self.evaluate_predicate(e, get_value))
                .fold(ThreeValuedBool::True, |acc, val| acc.and(val)),
            BoolExpr::Not(inner) => self.evaluate_predicate(inner, get_value).not(),
        }
    }

    /// Check if an identifier is explicitly false for this expansion
    fn is_ident_explicitly_false(&self, ident: &str) -> bool {
        // Check matrix values for explicit false
        if let Some(&value) = self.matrix_values.get(ident) {
            return !value;
        }

        // Check ID variant identifiers for explicit false
        if let Some(ref variant) = self.id_variant {
            match variant {
                KindVariant::IdU64 if ident == "id_uuid" => return true,
                KindVariant::IdUuid if ident == "id_u64" => return true,
                _ => {}
            }
        }

        false
    }

    /// Check if this expansion has a non-empty name (i.e., has id() or matrix() parameters)
    pub fn has_expansion(&self) -> bool {
        !self.name().is_empty()
    }

    /// Generate a syn::Ident for this expansion's name
    pub fn to_ident(&self) -> Option<syn::Ident> {
        let name = self.name();
        if name.is_empty() {
            None
        } else {
            Some(syn::Ident::new(&name, proc_macro2::Span::call_site()))
        }
    }
}

/// A boolean expression for requires() evaluation
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BoolExpr {
    /// A single identifier (e.g., "single", "auto_increment")
    Ident(String),
    /// Logical OR of sub-expressions
    Or(Vec<BoolExpr>),
    /// Logical AND of sub-expressions
    And(Vec<BoolExpr>),
    /// Logical NOT of a sub-expression
    Not(Box<BoolExpr>),
}

/// Type variants for ID replacement
#[derive(Debug, Clone)]
pub enum KindVariant {
    /// u64 ID type variant
    IdU64,
    /// UUID ID type variant
    IdUuid,
}

impl KindVariant {
    /// Get the variant function name (e.g., "id_u64")
    pub fn name(&self) -> &'static str {
        match self {
            KindVariant::IdU64 => "id_u64",
            KindVariant::IdUuid => "id_uuid",
        }
    }
}

/// Attribute arguments for `#[driver_test(...)]`
#[derive(Debug, Clone)]
pub struct DriverTestAttr {
    pub id_ident: Option<String>,
    pub matrix: Vec<String>,
    pub requires: Option<BoolExpr>,
    /// The original syn::Attribute
    pub ast: syn::Attribute,
}

impl DriverTestAttr {
    /// Parse from a syn::Attribute
    pub fn from_attribute(attr: &syn::Attribute) -> syn::Result<Self> {
        if attr.meta.require_path_only().is_ok() {
            // Empty attribute: #[driver_test]
            Ok(DriverTestAttr {
                id_ident: None,
                matrix: Vec::new(),
                requires: None,
                ast: attr.clone(),
            })
        } else {
            // Parse attribute arguments: #[driver_test(id(ID), requires(...))]
            let mut parsed = attr.parse_args::<DriverTestAttr>()?;
            parsed.ast = attr.clone();
            Ok(parsed)
        }
    }
}

impl Parse for DriverTestAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Capture the input as tokens to reconstruct the attribute
        let input_tokens = input.cursor().token_stream();

        let mut id_ident = None;
        let mut matrix = Vec::new();
        let mut requires = None;

        // Parse comma-separated list of attributes
        let attrs = Punctuated::<DriverTestAttrItem, Comma>::parse_terminated(input)?;

        for attr in attrs {
            match attr {
                DriverTestAttrItem::Id(ident) => {
                    id_ident = Some(ident);
                }
                DriverTestAttrItem::Matrix(items) => {
                    matrix = items;
                }
                DriverTestAttrItem::Requires(expr) => {
                    requires = Some(expr);
                }
            }
        }

        // Reconstruct the full attribute from the parsed tokens
        let ast = syn::parse_quote! { #[driver_test(#input_tokens)] };

        Ok(DriverTestAttr {
            id_ident,
            matrix,
            requires,
            ast,
        })
    }
}

/// Individual attribute item
#[derive(Debug)]
enum DriverTestAttrItem {
    /// id(IDENT) - specifies test should be expanded for multiple ID types
    Id(String),
    /// matrix(ident1, ident2, ...) - specifies custom matrix dimensions
    Matrix(Vec<String>),
    /// requires(expr) - specifies boolean expression for filtering expansions
    Requires(BoolExpr),
}

impl Parse for DriverTestAttrItem {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;

        match name.to_string().as_str() {
            "id" => {
                // Parse id(IDENT)
                let content;
                syn::parenthesized!(content in input);
                let ident: Ident = content.parse()?;
                Ok(DriverTestAttrItem::Id(ident.to_string()))
            }
            "matrix" => {
                // Parse matrix(ident1, ident2, ...)
                let content;
                syn::parenthesized!(content in input);
                let idents = Punctuated::<Ident, Comma>::parse_terminated(&content)?;
                Ok(DriverTestAttrItem::Matrix(
                    idents.into_iter().map(|i| i.to_string()).collect(),
                ))
            }
            "requires" => {
                // Parse requires(bool_expr)
                let content;
                syn::parenthesized!(content in input);
                let expr = BoolExpr::parse(&content)?;
                Ok(DriverTestAttrItem::Requires(expr))
            }
            _ => Err(syn::Error::new_spanned(
                name,
                "unknown attribute, expected `id`, `matrix`, or `requires`",
            )),
        }
    }
}

impl BoolExpr {
    /// Parse a boolean expression
    pub fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        let name = ident.to_string();

        match name.as_str() {
            "or" => {
                // Parse or(expr1, expr2, ...)
                let content;
                syn::parenthesized!(content in input);
                let mut exprs = Vec::new();
                while !content.is_empty() {
                    exprs.push(BoolExpr::parse(&content)?);
                    if content.is_empty() {
                        break;
                    }
                    content.parse::<Comma>()?;
                }
                Ok(BoolExpr::Or(exprs))
            }
            "and" => {
                // Parse and(expr1, expr2, ...)
                let content;
                syn::parenthesized!(content in input);
                let mut exprs = Vec::new();
                while !content.is_empty() {
                    exprs.push(BoolExpr::parse(&content)?);
                    if content.is_empty() {
                        break;
                    }
                    content.parse::<Comma>()?;
                }
                Ok(BoolExpr::And(exprs))
            }
            "not" => {
                // Parse not(expr)
                let content;
                syn::parenthesized!(content in input);
                let expr = BoolExpr::parse(&content)?;
                Ok(BoolExpr::Not(Box::new(expr)))
            }
            _ => {
                // Plain identifier
                Ok(BoolExpr::Ident(name))
            }
        }
    }
}

impl DriverTest {
    /// Parse a function with the `#[driver_test]` attribute
    pub fn from_item_fn(mut input: ItemFn, attr: DriverTestAttr) -> Self {
        let name = input.sig.ident.clone();
        let requires = attr.requires.clone();

        // Collect non-driver_test attributes to preserve them
        let attrs: Vec<_> = input
            .attrs
            .iter()
            .filter(|a| !a.path().is_ident("driver_test"))
            .cloned()
            .collect();

        // Remove the #[driver_test] attribute from the function, but keep all other attributes
        // (e.g., #[should_panic], #[ignore], etc.)
        input.attrs.retain(|a| !a.path().is_ident("driver_test"));

        // Generate expansion matrix
        let expansions = Self::generate_expansions(&attr);

        DriverTest {
            input,
            name,
            expansions,
            requires,
            attrs,
            attr,
        }
    }

    /// Generate all expansions based on the attribute
    pub(crate) fn generate_expansions(attr: &DriverTestAttr) -> Vec<Expansion> {
        let has_id = attr.id_ident.is_some();
        let has_matrix = !attr.matrix.is_empty();
        let has_requires = attr.requires.is_some();

        // If no expansions needed, return empty
        if !has_id && !has_matrix && !has_requires {
            return vec![];
        }

        let mut expansions = Vec::new();

        // ID variants to iterate over
        let id_variants = if has_id {
            vec![Some(KindVariant::IdU64), Some(KindVariant::IdUuid)]
        } else {
            vec![None]
        };

        // Generate all combinations of matrix values
        let matrix_combinations = Self::generate_matrix_combinations(&attr.matrix);

        // Combine ID variants with matrix combinations
        for id_variant in &id_variants {
            for matrix_values in &matrix_combinations {
                // Build the predicate for this expansion
                let predicate = if matches!(id_variant, Some(KindVariant::IdU64)) {
                    // For id_u64 expansions, add auto_increment to the predicate
                    let auto_increment = BoolExpr::Ident("auto_increment".to_string());
                    match &attr.requires {
                        Some(existing) => {
                            Some(BoolExpr::And(vec![existing.clone(), auto_increment]))
                        }
                        None => Some(auto_increment),
                    }
                } else {
                    // For non-id_u64 expansions, use the original predicate
                    attr.requires.clone()
                };

                let expansion = Expansion {
                    id_variant: id_variant.clone(),
                    id_ident: attr.id_ident.clone(),
                    matrix_values: matrix_values.clone(),
                    predicate,
                };

                // Filter using should_include with three-valued logic
                // Capabilities are unknown at compile time
                if !expansion.should_include(|_ident| ThreeValuedBool::Unknown) {
                    continue; // Skip this expansion
                }

                expansions.push(expansion);
            }
        }

        expansions
    }

    /// Generate all combinations of matrix values
    /// For matrix(a, b, c), this generates combinations where exactly one is true:
    /// - {a: true, b: false, c: false}
    /// - {a: false, b: true, c: false}
    /// - {a: false, b: false, c: true}
    fn generate_matrix_combinations(
        matrix: &[String],
    ) -> Vec<std::collections::HashMap<String, bool>> {
        if matrix.is_empty() {
            return vec![std::collections::HashMap::new()];
        }

        let mut combinations = Vec::new();

        // Generate one combination for each matrix value where only that value is true
        for (i, _key) in matrix.iter().enumerate() {
            let mut combination = std::collections::HashMap::new();
            for (j, other_key) in matrix.iter().enumerate() {
                combination.insert(other_key.clone(), i == j);
            }
            combinations.push(combination);
        }

        combinations
    }
}
