use std::sync::Arc;

use toasty_core::{
    driver::Capability,
    stmt::{self, Direction, Expr, Limit, LimitCursor, OrderByExpr, Statement},
};

use crate as toasty;
use crate::{
    engine::{Engine, test_util::test_schema_with},
    schema::Model,
};

#[test]
fn normalization_is_idempotent() {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,
        group: i64,
    }

    let schema = Arc::new(test_schema_with(&[Item::schema()]));
    let engine = Engine::new(schema, &Capability::SQLITE);
    let mut query = stmt::Query::new_select(Item::id(), true);
    query.order_by = Some(
        OrderByExpr {
            expr: Expr::ref_self_field(Item::id().field(1)),
            order: Some(Direction::Asc),
        }
        .into(),
    );
    query.limit = Some(Limit::Cursor(LimitCursor {
        page_size: 2_i64.into(),
        after: None,
    }));
    let mut stmt = Statement::Query(query);

    engine.normalize_stmt(&mut stmt).unwrap();
    let once = stmt.clone();
    engine.normalize_stmt(&mut stmt).unwrap();

    assert_eq!(stmt, once);
}
