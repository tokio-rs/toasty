use proc_macro2::Span;
use syn::parse::{Parse, ParseStream};
use syn::token;

/// Top-level input to the `query!` macro.
///
/// ```text
/// query!($source [FILTER $filter] [ORDER BY $field ASC|DESC] [OFFSET $n] [LIMIT $n])
/// ```
pub(crate) struct QueryInput {
    /// The model type path (e.g. `User`, `my_mod::User`).
    pub source: syn::Path,
    /// Optional filter expression.
    pub filter: Option<Expr>,
    /// Optional ORDER BY clause.
    pub order_by: Option<OrderByClause>,
    /// Optional OFFSET value.
    pub offset: Option<OffsetExpr>,
    /// Optional LIMIT value.
    pub limit: Option<LimitExpr>,
}

/// An ORDER BY clause: a field path and a direction.
#[derive(Debug)]
pub(crate) struct OrderByClause {
    pub field: FieldPath,
    pub direction: OrderDirection,
}

/// Sort direction for ORDER BY.
#[derive(Debug, Clone, Copy)]
pub(crate) enum OrderDirection {
    Asc,
    Desc,
}

/// A LIMIT expression — either a literal integer or an external reference.
#[derive(Debug)]
pub(crate) enum LimitExpr {
    Lit(syn::LitInt),
    Var(syn::Ident),
    RustExpr(Box<syn::Expr>),
}

/// An OFFSET expression — either a literal integer or an external reference.
#[derive(Debug)]
pub(crate) enum OffsetExpr {
    Lit(syn::LitInt),
    Var(syn::Ident),
    RustExpr(Box<syn::Expr>),
}

/// An expression — a tree of boolean operators, comparisons, NOT, and
/// parenthesized sub-expressions. Parsed with standard precedence:
///
/// 1. `OR` (lowest)
/// 2. `AND`
/// 3. `NOT` (prefix unary)
/// 4. Atoms: comparisons, parenthesized groups, literals, variables, field paths
#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Expr {
    /// `lhs AND rhs`
    And(Box<Expr>, Box<Expr>),
    /// `lhs OR rhs`
    Or(Box<Expr>, Box<Expr>),
    /// `NOT expr`
    Not(Box<Expr>),
    /// `lhs op rhs` (binary comparison)
    BinaryOp(ExprBinaryOp),
    /// `( expr )` — already folded into the tree during parsing, but kept as a
    /// variant so expansion can distinguish if needed in the future.
    Paren(Box<Expr>),
    /// A dot-prefixed field path: `.name`, `.profile.bio`, etc.
    Field(FieldPath),
    /// A literal: string, integer, float, bool.
    Lit(syn::Lit),
    /// `#ident` — a variable from the surrounding scope.
    Var(syn::Ident),
    /// `#(expr)` — an arbitrary Rust expression.
    RustExpr(Box<syn::Expr>),
}

/// A binary comparison: `lhs op rhs`.
#[derive(Debug)]
pub(crate) struct ExprBinaryOp {
    pub lhs: Box<Expr>,
    pub op: CompareOp,
    pub rhs: Box<Expr>,
}

/// A dot-prefixed field path: `.name`, `.profile.bio`, etc.
#[derive(Debug)]
pub(crate) struct FieldPath {
    /// Span of the leading `.` — reserved for future error reporting.
    pub _dot_span: Span,
    pub segments: Vec<syn::Ident>,
}

/// Comparison operator with its source span.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CompareOp {
    pub kind: CompareOpKind,
    /// Reserved for future error reporting.
    pub _span: Span,
}

/// Comparison operator kinds.
#[derive(Debug, Clone, Copy)]
pub(crate) enum CompareOpKind {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
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

        let order_by = if is_keyword(input, "order") {
            consume_ident(input)?;
            if !is_keyword(input, "by") {
                return Err(input.error("expected `BY` after `ORDER`"));
            }
            consume_ident(input)?;
            Some(parse_order_by(input)?)
        } else {
            None
        };

        let offset = if is_keyword(input, "offset") {
            consume_ident(input)?;
            Some(parse_pagination_expr::<OffsetExpr>(input)?)
        } else {
            None
        };

        let limit = if is_keyword(input, "limit") {
            consume_ident(input)?;
            Some(parse_pagination_expr::<LimitExpr>(input)?)
        } else {
            None
        };

        if !input.is_empty() {
            return Err(input.error("unexpected tokens after query"));
        }

        Ok(QueryInput {
            source,
            filter,
            order_by,
            offset,
            limit,
        })
    }
}

// ---------------------------------------------------------------------------
// Precedence-climbing filter parser
// ---------------------------------------------------------------------------

/// Parse an OR expression (lowest precedence).
fn parse_or(input: ParseStream) -> syn::Result<Expr> {
    let mut lhs = parse_and(input)?;
    while is_keyword(input, "or") {
        consume_ident(input)?;
        let rhs = parse_and(input)?;
        lhs = Expr::Or(Box::new(lhs), Box::new(rhs));
    }
    Ok(lhs)
}

/// Parse an AND expression.
fn parse_and(input: ParseStream) -> syn::Result<Expr> {
    let mut lhs = parse_unary(input)?;
    while is_keyword(input, "and") {
        consume_ident(input)?;
        let rhs = parse_unary(input)?;
        lhs = Expr::And(Box::new(lhs), Box::new(rhs));
    }
    Ok(lhs)
}

/// Parse a NOT prefix or an atom.
fn parse_unary(input: ParseStream) -> syn::Result<Expr> {
    if is_keyword(input, "not") {
        consume_ident(input)?;
        let expr = parse_unary(input)?;
        Ok(Expr::Not(Box::new(expr)))
    } else {
        parse_atom(input)
    }
}

/// Parse an atom: parenthesized group or comparison.
fn parse_atom(input: ParseStream) -> syn::Result<Expr> {
    if input.peek(token::Paren) {
        let content;
        syn::parenthesized!(content in input);
        let inner = parse_or(&content)?;
        if !content.is_empty() {
            return Err(content.error("unexpected tokens inside parentheses"));
        }
        Ok(Expr::Paren(Box::new(inner)))
    } else {
        let lhs = parse_primary(input)?;

        // If followed by a comparison operator, parse as binary op
        if peek_compare_op(input) {
            let op = parse_compare_op(input)?;
            let rhs = parse_primary(input)?;
            Ok(Expr::BinaryOp(ExprBinaryOp {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            }))
        } else {
            Ok(lhs)
        }
    }
}

/// Parse a primary expression: field path, literal, variable, or Rust expression.
fn parse_primary(input: ParseStream) -> syn::Result<Expr> {
    if input.peek(syn::Token![.]) {
        let path = parse_field_path(input)?;
        Ok(Expr::Field(path))
    } else if input.peek(syn::Token![#]) {
        // External reference: `#ident` or `#(expr)`
        input.parse::<syn::Token![#]>()?;
        if input.peek(token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            let expr: syn::Expr = content.parse()?;
            Ok(Expr::RustExpr(Box::new(expr)))
        } else {
            let ident: syn::Ident = input.parse()?;
            Ok(Expr::Var(ident))
        }
    } else if input.peek(syn::Lit) {
        let lit: syn::Lit = input.parse()?;
        Ok(Expr::Lit(lit))
    } else if is_keyword(input, "true") {
        let ident: syn::Ident = input.parse()?;
        Ok(Expr::Lit(syn::Lit::Bool(syn::LitBool {
            value: true,
            span: ident.span(),
        })))
    } else if is_keyword(input, "false") {
        let ident: syn::Ident = input.parse()?;
        Ok(Expr::Lit(syn::Lit::Bool(syn::LitBool {
            value: false,
            span: ident.span(),
        })))
    } else {
        Err(input.error("expected `.field`, literal, `#variable`, `#(expression)`, `NOT`, or `(`"))
    }
}

// ---------------------------------------------------------------------------
// Field paths
// ---------------------------------------------------------------------------

/// Parse a dot-prefixed field path: `.ident` (`.ident)*`.
fn parse_field_path(input: ParseStream) -> syn::Result<FieldPath> {
    let mut segments = Vec::new();

    // Consume the leading `.`
    let dot: syn::Token![.] = input.parse()?;
    let dot_span = dot.span;
    segments.push(input.parse::<syn::Ident>()?);

    // Consume additional `.ident` segments
    while input.peek(syn::Token![.]) && input.peek2(syn::Ident) {
        input.parse::<syn::Token![.]>()?;
        segments.push(input.parse::<syn::Ident>()?);
    }

    Ok(FieldPath {
        _dot_span: dot_span,
        segments,
    })
}

// ---------------------------------------------------------------------------
// Comparison operators
// ---------------------------------------------------------------------------

/// Check if the next token is a comparison operator without consuming it.
fn peek_compare_op(input: ParseStream) -> bool {
    input.peek(syn::Token![==])
        || input.peek(syn::Token![!=])
        || input.peek(syn::Token![>=])
        || input.peek(syn::Token![<=])
        || input.peek(syn::Token![>])
        || input.peek(syn::Token![<])
}

fn parse_compare_op(input: ParseStream) -> syn::Result<CompareOp> {
    if input.peek(syn::Token![==]) {
        let tok = input.parse::<syn::Token![==]>()?;
        Ok(CompareOp {
            kind: CompareOpKind::Eq,
            _span: tok.spans[0],
        })
    } else if input.peek(syn::Token![!=]) {
        let tok = input.parse::<syn::Token![!=]>()?;
        Ok(CompareOp {
            kind: CompareOpKind::Ne,
            _span: tok.spans[0],
        })
    } else if input.peek(syn::Token![>=]) {
        let tok = input.parse::<syn::Token![>=]>()?;
        Ok(CompareOp {
            kind: CompareOpKind::Ge,
            _span: tok.spans[0],
        })
    } else if input.peek(syn::Token![<=]) {
        let tok = input.parse::<syn::Token![<=]>()?;
        Ok(CompareOp {
            kind: CompareOpKind::Le,
            _span: tok.spans[0],
        })
    } else if input.peek(syn::Token![>]) {
        let tok = input.parse::<syn::Token![>]>()?;
        Ok(CompareOp {
            kind: CompareOpKind::Gt,
            _span: tok.span,
        })
    } else if input.peek(syn::Token![<]) {
        let tok = input.parse::<syn::Token![<]>()?;
        Ok(CompareOp {
            kind: CompareOpKind::Lt,
            _span: tok.span,
        })
    } else {
        Err(input.error("expected comparison operator (==, !=, >, >=, <, <=)"))
    }
}

// ---------------------------------------------------------------------------
// ORDER BY, OFFSET, LIMIT helpers
// ---------------------------------------------------------------------------

/// Parse `ORDER BY .field ASC|DESC`.
fn parse_order_by(input: ParseStream) -> syn::Result<OrderByClause> {
    let field = parse_field_path(input)?;

    let direction = if is_keyword(input, "asc") {
        consume_ident(input)?;
        OrderDirection::Asc
    } else if is_keyword(input, "desc") {
        consume_ident(input)?;
        OrderDirection::Desc
    } else {
        // Default to ascending if no direction specified
        OrderDirection::Asc
    };

    Ok(OrderByClause { field, direction })
}

/// Trait to construct pagination expressions from parsed tokens.
trait PaginationExpr: Sized {
    fn from_lit(lit: syn::LitInt) -> Self;
    fn from_var(ident: syn::Ident) -> Self;
    fn from_rust_expr(expr: Box<syn::Expr>) -> Self;
}

impl PaginationExpr for LimitExpr {
    fn from_lit(lit: syn::LitInt) -> Self {
        LimitExpr::Lit(lit)
    }
    fn from_var(ident: syn::Ident) -> Self {
        LimitExpr::Var(ident)
    }
    fn from_rust_expr(expr: Box<syn::Expr>) -> Self {
        LimitExpr::RustExpr(expr)
    }
}

impl PaginationExpr for OffsetExpr {
    fn from_lit(lit: syn::LitInt) -> Self {
        OffsetExpr::Lit(lit)
    }
    fn from_var(ident: syn::Ident) -> Self {
        OffsetExpr::Var(ident)
    }
    fn from_rust_expr(expr: Box<syn::Expr>) -> Self {
        OffsetExpr::RustExpr(expr)
    }
}

/// Parse a pagination value: integer literal, `#ident`, or `#(expr)`.
fn parse_pagination_expr<T: PaginationExpr>(input: ParseStream) -> syn::Result<T> {
    if input.peek(syn::LitInt) {
        let lit: syn::LitInt = input.parse()?;
        Ok(T::from_lit(lit))
    } else if input.peek(syn::Token![#]) {
        input.parse::<syn::Token![#]>()?;
        if input.peek(token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            let expr: syn::Expr = content.parse()?;
            Ok(T::from_rust_expr(Box::new(expr)))
        } else {
            let ident: syn::Ident = input.parse()?;
            Ok(T::from_var(ident))
        }
    } else {
        Err(input.error("expected integer literal, `#variable`, or `#(expression)`"))
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
