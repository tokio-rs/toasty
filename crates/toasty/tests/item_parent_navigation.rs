//! Compile-time + AST-level exercise for `child.parent()` navigation
//! on `#[item_parent]` fields (B4.8).
//!
//! Goal: assert that the macro emits the right query shape for
//! `user.tenant()` — a partition-equality filter on the child's
//! partition column plus a sort-key prefix filter `"<Parent>#"` on the
//! parent's sort column (design R2.9). End-to-end runtime behaviour
//! against a real driver is deferred to B4.10 verification, which
//! depends on shared-table column-reuse work that lands in B4.9.
//!
//! The test loads a User from a synthetic record (no Db needed),
//! invokes `user.tenant()`, and walks the resulting `One<Tenant>`
//! statement to confirm the filter has the expected operands.

use toasty::stmt::IntoStatement;
use toasty::{Deferred, schema::Load};
use toasty_core::stmt::{self, Value};

#[derive(Debug, toasty::Model)]
#[key(account, sk)]
struct Tenant {
    account: String,
    #[auto]
    sk: String,
    name: String,
}

#[derive(Debug, toasty::Model)]
#[key(account, sk)]
struct User {
    account: String,
    #[auto]
    sk: String,
    name: String,
    #[item_parent]
    tenant: Deferred<Tenant>,
}

/// Build a User record matching the schema's field order
/// (account, sk, name, tenant). The tenant slot is left null — the
/// emitted accessor must read its identity through schema metadata,
/// not through the marker field's runtime value.
fn make_user(account: &str, sk: &str, name: &str) -> User {
    let record = Value::record_from_vec(vec![
        Value::String(account.to_string()),
        Value::String(sk.to_string()),
        Value::String(name.to_string()),
        Value::Null,
    ]);
    User::load(record).expect("user load round-trips a literal record")
}

/// `user.tenant()` lowers to a `Tenant`-rooted query whose filter is
/// `Tenant.account == self.account AND Tenant.sk STARTS_WITH "Tenant#"`.
/// Uses field-name lookup to avoid coupling the test to in-source field
/// ordering.
#[test]
fn user_tenant_emits_partition_eq_and_sort_prefix_filter() {
    let user = make_user("acme", "User#01", "Alice");
    let one = user.tenant();
    let stmt = one.into_statement();
    let stmt::Statement::Query(query) = stmt.into_untyped() else {
        panic!("tenant() yields a query");
    };
    let body_select = query.body.as_select().expect("query body is a Select");
    let filter = body_select
        .filter
        .expr
        .as_ref()
        .expect("filter is populated");

    // Drill into the `And(eq, starts_with)` shape.
    let stmt::Expr::And(operands) = filter else {
        panic!("expected an `And` filter, got {filter:#?}");
    };
    assert_eq!(operands.len(), 2, "expected exactly two filter operands");

    // Operand 0: account == "acme".
    let stmt::Expr::BinaryOp(eq) = &operands[0] else {
        panic!("expected eq operand at index 0, got {:#?}", operands[0]);
    };
    assert!(eq.op.is_eq(), "first operand must be `==`");
    assert!(
        matches!(&*eq.rhs, stmt::Expr::Value(Value::String(s)) if s == "acme"),
        "expected RHS = String(\"acme\"), got {:#?}",
        eq.rhs
    );

    // Operand 1: sk STARTS_WITH "Tenant#".
    let stmt::Expr::StartsWith(sw) = &operands[1] else {
        panic!(
            "expected starts_with operand at index 1, got {:#?}",
            operands[1]
        );
    };
    assert!(
        matches!(&*sw.prefix, stmt::Expr::Value(Value::String(s)) if s == "Tenant#"),
        "expected prefix = String(\"Tenant#\"), got {:#?}",
        sw.prefix
    );
}
