use toasty_core::stmt::{self, VisitMut};

use super::LowerStatement;

impl LowerStatement<'_, '_> {
    pub(super) fn lower_expr_starts_with(&mut self, expr: &mut stmt::Expr) {
        // SQL drivers without a native starts_with rewrite to
        // `Like(expr, prefix || '%')`. If the prefix contains LIKE wildcards
        // (`%` or `_`) we pick an escape character that doesn't appear in
        // the prefix, prepend it to each wildcard, and emit
        // `LIKE 'pattern' ESCAPE 'c'`. If the prefix is wildcard-free, no
        // escape clause is needed and the AST stays free of one — matching
        // what a hand-written `Path::like("foo%")` would produce.
        assert!(
            self.capability().native_like,
            "lowering starts_with to LIKE requires native_like capability",
        );

        let stmt::Expr::StartsWith(mut e) = expr.take() else {
            panic!()
        };

        self.visit_expr_mut(&mut e.expr);
        self.visit_expr_mut(&mut e.prefix);

        let stmt::Expr::Value(stmt::Value::String(s)) = *e.prefix else {
            panic!("unexpected StartsWith prefix expression: {:?}", e.prefix);
        };

        let escape = pick_escape_char(&s);
        let mut pattern = String::with_capacity(s.len() + 1);
        for c in s.chars() {
            if let Some(esc) = escape
                && (c == '%' || c == '_')
            {
                pattern.push(esc);
            }
            pattern.push(c);
        }
        pattern.push('%');

        *expr = stmt::ExprLike {
            expr: Box::new(*e.expr),
            pattern: Box::new(stmt::Expr::Value(stmt::Value::String(pattern))),
            escape,
            case_insensitive: false,
        }
        .into();
    }
}

/// Pick a `LIKE` escape character for a starts_with prefix, or `None` if no
/// escape is needed (the prefix has no `%` or `_` wildcards).
///
/// The chosen char must not appear in the prefix — that way it never
/// accidentally escapes a literal character, and we don't have to
/// double-escape the escape char itself.
fn pick_escape_char(prefix: &str) -> Option<char> {
    if !prefix.contains('%') && !prefix.contains('_') {
        return None;
    }
    // Try common, visually-quiet ASCII candidates first. Almost every prefix
    // hits one of these on the first try.
    for c in ['!', '~', '#', '@', '|', '^', '`', '\\'] {
        if !prefix.contains(c) {
            return Some(c);
        }
    }
    // Pathological prefix containing every common candidate — fall back to
    // a low-ASCII control character. The codepoint range below excludes
    // NUL, tab, and the line terminators, leaving plenty of choices.
    for c in '\x01'..='\x08' {
        if !prefix.contains(c) {
            return Some(c);
        }
    }
    panic!("could not find a LIKE escape character for prefix {prefix:?}");
}

#[cfg(test)]
mod tests {
    use super::pick_escape_char;

    #[test]
    fn no_wildcards_returns_none() {
        assert_eq!(pick_escape_char(""), None);
        assert_eq!(pick_escape_char("alpha"), None);
        assert_eq!(pick_escape_char("café\\!"), None);
    }

    #[test]
    fn first_unused_candidate_wins() {
        assert_eq!(pick_escape_char("100%"), Some('!'));
        assert_eq!(pick_escape_char("a_b"), Some('!'));
        assert_eq!(pick_escape_char("!100%"), Some('~'));
        assert_eq!(pick_escape_char("!~100%"), Some('#'));
    }
}
