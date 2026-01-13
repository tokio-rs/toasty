use syn::{parse::Parse, punctuated::Punctuated, token::Comma, Ident, ItemFn};

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

    /// Whether the test had id() or matrix() parameters (even if all expansions filtered out)
    pub has_expansion_params: bool,
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
}

/// A capability requirement, optionally negated
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Capability {
    /// The capability name
    pub name: String,
    /// Whether this capability should NOT be present
    pub negated: bool,
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

/// Kinds of test variants to generate
#[derive(Debug, Clone)]
pub struct Kind {
    /// The identifier to replace (e.g., "ID")
    pub ident: String,
    /// The target type variant
    pub variant: KindVariant,
}

/// Type variants for ID replacement
#[derive(Debug, Clone)]
pub enum KindVariant {
    /// u64 ID type variant
    IdU64,
    /// UUID ID type variant
    IdUuid,
}

impl Kind {
    /// Get the variant function name (e.g., "id_u64")
    pub fn name(&self) -> &'static str {
        self.variant.name()
    }

    /// Get the target type for ID replacement
    pub fn target_type(&self) -> syn::Type {
        self.variant.target_type()
    }

    /// Get the identifier to replace
    pub fn ident(&self) -> &str {
        &self.ident
    }
}

impl KindVariant {
    /// Get the variant function name (e.g., "id_u64")
    pub fn name(&self) -> &'static str {
        match self {
            KindVariant::IdU64 => "id_u64",
            KindVariant::IdUuid => "id_uuid",
        }
    }

    /// Get the target type for ID replacement
    pub fn target_type(&self) -> syn::Type {
        match self {
            KindVariant::IdU64 => syn::parse_quote!(u64),
            KindVariant::IdUuid => syn::parse_quote!(uuid::Uuid),
        }
    }
}

/// Attribute arguments for `#[driver_test(...)]`
#[derive(Debug)]
pub struct DriverTestAttr {
    pub id_ident: Option<String>,
    pub matrix: Vec<String>,
    pub requires: Option<BoolExpr>,
}

impl Parse for DriverTestAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
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

        Ok(DriverTestAttr {
            id_ident,
            matrix,
            requires,
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

    /// Evaluate the boolean expression given a set of true identifiers
    pub fn eval<F>(&self, is_true: &F) -> bool
    where
        F: Fn(&str) -> bool,
    {
        match self {
            BoolExpr::Ident(name) => is_true(name),
            BoolExpr::Or(exprs) => exprs.iter().any(|e| e.eval(is_true)),
            BoolExpr::And(exprs) => exprs.iter().all(|e| e.eval(is_true)),
            BoolExpr::Not(expr) => !expr.eval(is_true),
        }
    }
}

impl DriverTest {
    /// Parse a function with the `#[driver_test]` attribute
    pub fn from_item_fn(mut input: ItemFn, attr: DriverTestAttr) -> Self {
        let name = input.sig.ident.clone();
        let requires = attr.requires.clone();
        let has_expansion_params = attr.id_ident.is_some() || !attr.matrix.is_empty();

        // Collect non-driver_test attributes to preserve them
        let attrs: Vec<_> = input
            .attrs
            .iter()
            .filter(|attr| !attr.path().is_ident("driver_test"))
            .cloned()
            .collect();

        // Remove the #[driver_test] attribute from the function, but keep all other attributes
        // (e.g., #[should_panic], #[ignore], etc.)
        input
            .attrs
            .retain(|attr| !attr.path().is_ident("driver_test"));

        // Generate expansion matrix
        let expansions = Self::generate_expansions(&attr);

        DriverTest {
            input,
            name,
            expansions,
            requires,
            attrs,
            has_expansion_params,
        }
    }

    /// Generate all expansions based on the attribute
    fn generate_expansions(attr: &DriverTestAttr) -> Vec<Expansion> {
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
                let expansion = Expansion {
                    id_variant: id_variant.clone(),
                    id_ident: attr.id_ident.clone(),
                    matrix_values: matrix_values.clone(),
                };

                // Filter by requires expression if present
                // Only evaluate with respect to matrix/ID identifiers
                // Database capabilities are checked at runtime, not compile time
                if let Some(ref requires_expr) = attr.requires {
                    let result = requires_expr.eval(&|ident| {
                        // Check if this is a matrix identifier
                        if expansion.matrix_values.contains_key(ident) {
                            expansion.matrix_values[ident]
                        } else if ident == "id_u64" || ident == "id_uuid" {
                            // ID variant identifier
                            if ident == "id_u64" {
                                matches!(
                                    expansion.id_variant,
                                    Some(crate::parse::KindVariant::IdU64)
                                )
                            } else {
                                matches!(
                                    expansion.id_variant,
                                    Some(crate::parse::KindVariant::IdUuid)
                                )
                            }
                        } else {
                            // Unknown identifier (database capability) - assume true for compile-time filtering
                            // The actual check will happen at runtime
                            true
                        }
                    });

                    if !result {
                        continue;
                    }
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
