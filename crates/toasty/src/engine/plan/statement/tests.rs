use super::*;
use crate::engine::{Engine, HirStatement, test_util::test_schema};
use toasty_core::driver::Capability;
use toasty_core::stmt::{Expr, ExprArg, ExprLet, Type, Value};

fn rewrite(expr: Expr) -> Option<Expr> {
    let engine = Engine::new(test_schema().into(), &Capability::SQLITE);
    let mut hir = HirStatement::new();
    let parent = hir.new_statement_info(IndexMap::new());
    let other = hir.new_statement_info(IndexMap::new());
    let column = |column| {
        stmt::ExprReference::Column(stmt::ExprColumn {
            nesting: 0,
            table: 0,
            column,
        })
    };
    let back_ref = hir::BackRef {
        exprs: [column(3), column(7)].into(),
        ..Default::default()
    };
    let mut child = hir::StatementInfo::new(IndexMap::new());
    for stmt_id in [parent, other] {
        child.args.push(hir::Arg::Ref {
            target_expr_ref: column(7),
            stmt_id,
            nesting: 1,
            data_load_input: Default::default(),
            returning_input: Default::default(),
            batch_load_index: Default::default(),
        });
    }
    child.args.push(hir::Arg::Sub {
        stmt_id: other,
        returning: false,
        input: Default::default(),
        batch_load_index: Default::default(),
    });
    let mut planner = HirPlanner {
        engine: &engine,
        hir: &hir,
        mir: mir::Store::new(),
    };
    PlanStatement {
        planner: &mut planner,
        stmt_id: parent,
        stmt_info: &hir[parent],
        load_data: LoadData {
            inputs: IndexSet::new(),
            select_items: SelectItems::new(),
            batch_load_args: IndexSet::new(),
        },
        remaining_deps: vec![],
    }
    .rewrite_parent_only_conjunct(&child, &back_ref, expr)
}

fn assert_parent_filter(expr: Expr) {
    let expr = rewrite(expr).expect("parent-only predicate");
    let func = eval::Func::from_stmt(expr, vec![Type::Record(vec![Type::I64, Type::I64])]);
    let schema = test_schema();
    for (value, expected) in [(42i64, true), (9, false)] {
        let row = Value::record_from_vec(vec![0i64.into(), value.into()]);
        assert_eq!(func.eval_bool(&schema, [row]).unwrap(), expected);
    }
}

fn scoped_arg(position: usize, nesting: usize) -> Expr {
    Expr::arg(ExprArg { position, nesting })
}

#[test]
fn parent_filter_supports_in_list() {
    assert_parent_filter(Expr::in_list(Expr::arg(0), Expr::list([42i64])));
}

#[test]
fn parent_filter_preserves_map_and_let_scopes() {
    assert_parent_filter(
        ExprLet {
            bindings: vec![Expr::arg(0)],
            body: Box::new(Expr::any(Expr::map(
                Expr::list([42i64]),
                Expr::and(
                    Expr::eq(Expr::arg(0), scoped_arg(0, 1)),
                    Expr::eq(Expr::arg(0), scoped_arg(0, 2)),
                ),
            ))),
        }
        .into(),
    );
}

#[test]
fn parent_filter_finds_parent_only_in_map_body() {
    assert_parent_filter(Expr::any(Expr::map(
        Expr::list([42i64]),
        Expr::eq(Expr::arg(0), scoped_arg(0, 1)),
    )));
}

#[test]
fn parent_filter_rejects_other_dependencies() {
    for dependency in [
        Expr::arg(1),
        Expr::arg(2),
        Expr::arg(3),
        scoped_arg(0, 1),
        Expr::Reference(stmt::ExprReference::Model { nesting: 0 }),
        Expr::any(Expr::map(Expr::list([true]), scoped_arg(1, 1))),
    ] {
        assert!(rewrite(Expr::and(Expr::arg(0), dependency)).is_none());
    }
}

#[test]
fn local_arguments_do_not_count_as_parent_references() {
    assert!(rewrite(Expr::any(Expr::map(Expr::list([true]), Expr::arg(0)))).is_none());
    assert!(
        rewrite(
            ExprLet {
                bindings: vec![Expr::from(false), Expr::from(true)],
                body: Box::new(Expr::arg(1)),
            }
            .into()
        )
        .is_none()
    );
}
