use super::{Effect, classify};
use toasty_core::schema::app::ModelId;
use toasty_core::schema::db::TableId;
use toasty_core::stmt::{
    self, Assignments, Condition, Delete, Expr, ExprStmt, Filter, Insert, InsertTarget, Query,
    Source, Statement, Update, UpdateTarget, With,
};

fn model() -> ModelId {
    ModelId(0)
}

fn simple_query() -> Query {
    Query::new_select(Source::from(model()), Filter::default())
}

fn simple_insert() -> Insert {
    Insert {
        target: InsertTarget::Model(model()),
        source: Query::unit(),
        upsert: None,
        returning: None,
    }
}

fn simple_update() -> Update {
    Update {
        target: UpdateTarget::Model(model()),
        assignments: Assignments::default(),
        filter: Filter::default(),
        condition: Condition::default(),
        returning: None,
    }
}

fn simple_delete() -> Delete {
    Delete {
        from: Source::from(model()),
        filter: Filter::default(),
        returning: None,
        condition: Condition::default(),
    }
}

fn query_with_cte(body: impl Into<stmt::ExprSet>) -> Statement {
    let mut query = simple_query();
    query.with = Some(With {
        ctes: vec![stmt::Cte {
            query: Query::new(body),
        }],
    });
    Statement::Query(query)
}

// --- top-level variants ---

#[test]
fn top_level_query_is_read_only() {
    let stmt = Statement::Query(simple_query());
    assert_eq!(classify(&stmt), Effect::ReadOnly);
}

#[test]
fn top_level_insert_is_mutating() {
    let stmt = Statement::Insert(simple_insert());
    assert_eq!(classify(&stmt), Effect::Mutating);
}

#[test]
fn top_level_update_is_mutating() {
    let stmt = Statement::Update(simple_update());
    assert_eq!(classify(&stmt), Effect::Mutating);
}

#[test]
fn top_level_delete_is_mutating() {
    let stmt = Statement::Delete(simple_delete());
    assert_eq!(classify(&stmt), Effect::Mutating);
}

// --- embedded mutation sub-statements ---

#[test]
fn query_with_insert_in_filter_is_mutating() {
    // SELECT ... WHERE id = (INSERT ... RETURNING id)
    // Build by injecting an Expr::Stmt(Insert) into the filter expression.
    let mut query = simple_query();
    let stmt::ExprSet::Select(select) = &mut query.body else {
        unreachable!()
    };
    select.filter = Filter::from(Expr::Stmt(ExprStmt {
        stmt: Box::new(Statement::Insert(simple_insert())),
    }));

    let stmt = Statement::Query(query);
    assert_eq!(classify(&stmt), Effect::Mutating);
}

#[test]
fn query_with_update_in_filter_is_mutating() {
    let mut query = simple_query();
    let stmt::ExprSet::Select(select) = &mut query.body else {
        unreachable!()
    };
    select.filter = Filter::from(Expr::Stmt(ExprStmt {
        stmt: Box::new(Statement::Update(simple_update())),
    }));

    let stmt = Statement::Query(query);
    assert_eq!(classify(&stmt), Effect::Mutating);
}

#[test]
fn query_with_delete_in_filter_is_mutating() {
    let mut query = simple_query();
    let stmt::ExprSet::Select(select) = &mut query.body else {
        unreachable!()
    };
    select.filter = Filter::from(Expr::Stmt(ExprStmt {
        stmt: Box::new(Statement::Delete(simple_delete())),
    }));

    let stmt = Statement::Query(query);
    assert_eq!(classify(&stmt), Effect::Mutating);
}

#[test]
fn query_with_nested_query_only_is_read_only() {
    // SELECT ... WHERE id IN (SELECT id FROM other) — a subquery that
    // doesn't mutate keeps the outer statement ReadOnly.
    let mut outer = simple_query();
    let stmt::ExprSet::Select(select) = &mut outer.body else {
        unreachable!()
    };
    select.filter = Filter::from(Expr::Stmt(ExprStmt {
        stmt: Box::new(Statement::Query(simple_query())),
    }));

    let stmt = Statement::Query(outer);
    assert_eq!(classify(&stmt), Effect::ReadOnly);
}

#[test]
fn query_with_cte_insert_is_mutating() {
    // WITH ins AS (INSERT ... RETURNING *) SELECT * FROM ins
    let stmt = query_with_cte(simple_insert());

    assert_eq!(classify(&stmt), Effect::Mutating);
}

#[test]
fn query_with_cte_update_is_mutating() {
    let stmt = query_with_cte(simple_update());

    assert_eq!(classify(&stmt), Effect::Mutating);
}

#[test]
fn query_with_cte_delete_is_mutating() {
    let stmt = query_with_cte(simple_delete());

    assert_eq!(classify(&stmt), Effect::Mutating);
}

#[test]
fn deeply_nested_mutation_is_mutating() {
    // SELECT ... WHERE id IN (SELECT id FROM ... WHERE x = (UPDATE ... RETURNING id))
    // Mutation buried two levels of subquery deep.
    let mut inner = simple_query();
    let stmt::ExprSet::Select(select) = &mut inner.body else {
        unreachable!()
    };
    select.filter = Filter::from(Expr::Stmt(ExprStmt {
        stmt: Box::new(Statement::Update(simple_update())),
    }));

    let mut outer = simple_query();
    let stmt::ExprSet::Select(select) = &mut outer.body else {
        unreachable!()
    };
    select.filter = Filter::from(Expr::Stmt(ExprStmt {
        stmt: Box::new(Statement::Query(inner)),
    }));

    let stmt = Statement::Query(outer);
    assert_eq!(classify(&stmt), Effect::Mutating);
}

// --- lowered shapes also classify correctly ---

#[test]
fn lowered_update_to_table_is_mutating() {
    // Post-lowering, UpdateTarget::Model becomes UpdateTarget::Table.
    // The top-level variant check still fires regardless of target shape.
    let stmt = Statement::Update(Update {
        target: UpdateTarget::Table(TableId(0)),
        assignments: Assignments::default(),
        filter: Filter::default(),
        condition: Condition::default(),
        returning: None,
    });
    assert_eq!(classify(&stmt), Effect::Mutating);
}
