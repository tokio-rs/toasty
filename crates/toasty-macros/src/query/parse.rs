use syn::parse::{Parse, ParseStream};
use syn::token;

/// Top-level input to the `query!` macro.
///
/// ```text
/// query!($source [FILTER $filter])
/// ```
///
/// Future clauses (ORDER BY, OFFSET, LIMIT, includes) will be added here.
pub(crate) struct QueryInput {
    /// The model type path (e.g. `User`, `my_mod::User`).
    pub source: syn::Path,
    /// Optional filter expression.
    pub filter: Option<FilterExpr>,
}

/// A filter expression — a tree of boolean operators, comparisons, NOT, and
/// parenthesized sub-expressions. Parsed with standard precedence:
///
/// 1. `OR` (lowest)
/// 2. `AND`
/// 3. `NOT` (prefix unary)
/// 4. Atoms: comparisons, parenthesized groups
#[derive(Debug)]
pub(crate) enum FilterExpr {
    /// `lhs AND rhs`
    And(Box<FilterExpr>, Box<FilterExpr>),
    /// `lhs OR rhs`
    Or(Box<FilterExpr>, Box<FilterExpr>),
    /// `NOT expr`
    Not(Box<FilterExpr>),
    /// `lhs op rhs` (comparison)
    Compare(CompareExpr),
    /// `( expr )` — already folded into the tree during parsing, but kept as a
    /// variant so expansion can distinguish if needed in the future.
    Paren(Box<FilterExpr>),
}

/// A binary comparison: `.field op value`.
#[derive(Debug)]
pub(crate) struct CompareExpr {
    pub lhs: FieldPath,
    pub op: CompareOp,
    pub rhs: Value,
}

/// A dot-prefixed field path: `.name`, `.profile.bio`, etc.
#[derive(Debug)]
pub(crate) struct FieldPath {
    pub segments: Vec<syn::Ident>,
}

/// Comparison operators.
#[derive(Debug, Clone, Copy)]
pub(crate) enum CompareOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

/// The right-hand side of a comparison.
#[derive(Debug)]
pub(crate) enum Value {
    /// A literal: string, integer, float, bool.
    Lit(syn::Lit),
    /// `#ident` — a variable from the surrounding scope.
    Var(syn::Ident),
    /// `#(expr)` — an arbitrary Rust expression.
    Expr(Box<syn::Expr>),
    /// A dot-prefixed field path (field-to-field comparison).
    Field(FieldPath),
}

// ---------------------------------------------------------------------------
// Top-level parse
// ---------------------------------------------------------------------------

impl Parse for QueryInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let source: syn::Path = input.parse()?;

        let filter = if is_keyword(input, "filter") {
            consume_ident(input)?;
            Some(parse_or(input)?)
        } else {
            None
        };

        if !input.is_empty() {
            return Err(input.error("unexpected tokens after query"));
        }

        Ok(QueryInput { source, filter })
    }
}

// ---------------------------------------------------------------------------
// Precedence-climbing filter parser
// ---------------------------------------------------------------------------

/// Parse an OR expression (lowest precedence).
fn parse_or(input: ParseStream) -> syn::Result<FilterExpr> {
    let mut lhs = parse_and(input)?;
    while is_keyword(input, "or") {
        consume_ident(input)?;
        let rhs = parse_and(input)?;
        lhs = FilterExpr::Or(Box::new(lhs), Box::new(rhs));
    }
    Ok(lhs)
}

/// Parse an AND expression.
fn parse_and(input: ParseStream) -> syn::Result<FilterExpr> {
    let mut lhs = parse_unary(input)?;
    while is_keyword(input, "and") {
        consume_ident(input)?;
        let rhs = parse_unary(input)?;
        lhs = FilterExpr::And(Box::new(lhs), Box::new(rhs));
    }
    Ok(lhs)
}

/// Parse a NOT prefix or an atom.
fn parse_unary(input: ParseStream) -> syn::Result<FilterExpr> {
    if is_keyword(input, "not") {
        consume_ident(input)?;
        let expr = parse_unary(input)?;
        Ok(FilterExpr::Not(Box::new(expr)))
    } else {
        parse_atom(input)
    }
}

/// Parse an atom: parenthesized group or comparison.
fn parse_atom(input: ParseStream) -> syn::Result<FilterExpr> {
    if input.peek(token::Paren) {
        let content;
        syn::parenthesized!(content in input);
        let inner = parse_or(&content)?;
        if !content.is_empty() {
            return Err(content.error("unexpected tokens inside parentheses"));
        }
        Ok(FilterExpr::Paren(Box::new(inner)))
    } else if input.peek(syn::Token![.]) {
        parse_comparison(input)
    } else {
        Err(input.error("expected `.field`, `NOT`, or `(` in filter expression"))
    }
}

/// Parse a comparison: `.field op value`.
fn parse_comparison(input: ParseStream) -> syn::Result<FilterExpr> {
    let lhs = parse_field_path(input)?;
    let op = parse_compare_op(input)?;
    let rhs = parse_value(input)?;
    Ok(FilterExpr::Compare(CompareExpr { lhs, op, rhs }))
}

// ---------------------------------------------------------------------------
// Field paths
// ---------------------------------------------------------------------------

/// Parse a dot-prefixed field path: `.ident` (`.ident)*`.
fn parse_field_path(input: ParseStream) -> syn::Result<FieldPath> {
    let mut segments = Vec::new();

    // Consume the leading `.`
    input.parse::<syn::Token![.]>()?;
    segments.push(input.parse::<syn::Ident>()?);

    // Consume additional `.ident` segments
    while input.peek(syn::Token![.]) && input.peek2(syn::Ident) {
        input.parse::<syn::Token![.]>()?;
        segments.push(input.parse::<syn::Ident>()?);
    }

    Ok(FieldPath { segments })
}

// ---------------------------------------------------------------------------
// Comparison operators
// ---------------------------------------------------------------------------

fn parse_compare_op(input: ParseStream) -> syn::Result<CompareOp> {
    if input.peek(syn::Token![==]) {
        input.parse::<syn::Token![==]>()?;
        Ok(CompareOp::Eq)
    } else if input.peek(syn::Token![!=]) {
        input.parse::<syn::Token![!=]>()?;
        Ok(CompareOp::Ne)
    } else if input.peek(syn::Token![>=]) {
        input.parse::<syn::Token![>=]>()?;
        Ok(CompareOp::Ge)
    } else if input.peek(syn::Token![<=]) {
        input.parse::<syn::Token![<=]>()?;
        Ok(CompareOp::Le)
    } else if input.peek(syn::Token![>]) {
        input.parse::<syn::Token![>]>()?;
        Ok(CompareOp::Gt)
    } else if input.peek(syn::Token![<]) {
        input.parse::<syn::Token![<]>()?;
        Ok(CompareOp::Lt)
    } else {
        Err(input.error("expected comparison operator (==, !=, >, >=, <, <=)"))
    }
}

// ---------------------------------------------------------------------------
// Values (RHS of comparison)
// ---------------------------------------------------------------------------

fn parse_value(input: ParseStream) -> syn::Result<Value> {
    if input.peek(syn::Token![.]) {
        // Field-to-field comparison
        let path = parse_field_path(input)?;
        Ok(Value::Field(path))
    } else if input.peek(syn::Token![#]) {
        // External reference: `#ident` or `#(expr)`
        input.parse::<syn::Token![#]>()?;
        if input.peek(token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            let expr: syn::Expr = content.parse()?;
            Ok(Value::Expr(Box::new(expr)))
        } else {
            let ident: syn::Ident = input.parse()?;
            Ok(Value::Var(ident))
        }
    } else if input.peek(syn::Lit) {
        let lit: syn::Lit = input.parse()?;
        Ok(Value::Lit(lit))
    } else if is_keyword(input, "true") {
        let ident: syn::Ident = input.parse()?;
        Ok(Value::Lit(syn::Lit::Bool(syn::LitBool {
            value: true,
            span: ident.span(),
        })))
    } else if is_keyword(input, "false") {
        let ident: syn::Ident = input.parse()?;
        Ok(Value::Lit(syn::Lit::Bool(syn::LitBool {
            value: false,
            span: ident.span(),
        })))
    } else {
        Err(input
            .error("expected a value: literal, `#variable`, `#(expression)`, or `.field` path"))
    }
}

// ---------------------------------------------------------------------------
// Keyword helpers — case-insensitive matching
// ---------------------------------------------------------------------------

/// Check if the next token is an identifier matching `kw` (case-insensitive),
/// without consuming it.
fn is_keyword(input: ParseStream, kw: &str) -> bool {
    input.peek(syn::Ident)
        && input
            .fork()
            .parse::<syn::Ident>()
            .map(|id| id.to_string().eq_ignore_ascii_case(kw))
            .unwrap_or(false)
}

/// Consume an identifier token (used after `is_keyword` confirmed it).
fn consume_ident(input: ParseStream) -> syn::Result<syn::Ident> {
    input.parse::<syn::Ident>()
}
