use toasty::schema::{
    app::FieldTy,
    mapping::{self, FieldEnum, FieldPrimitive},
};

use crate::{helpers::column, prelude::*};

use toasty_core::{
    driver::Operation,
    stmt::{BinaryOp, Expr, ExprSet, Statement},
};

/// Tests basic CRUD operations with an embedded enum field.
/// Validates create, read, update (both instance and query-based), and delete.
/// The enum discriminant is stored as an INTEGER column and reconstructed on load.
/// On SQL backends, also verifies the driver-level representation: column names and
/// discriminant values stored as I64 with no record wrapping.
#[driver_test(id(ID))]
pub async fn create_and_query_enum(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Status {
        #[column(variant = 1)]
        Pending,
        #[column(variant = 2)]
        Active,
        #[column(variant = 3)]
        Done,
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        status: Status,
    }

    let mut db = t.setup_db(models!(User, Status)).await;
    let user_table = table_id(&db, "users");

    // Create: enum variant is stored as its discriminant (1 = Pending)
    t.log().clear();

    let mut user = User::create()
        .name("Alice")
        .status(Status::Pending)
        .exec(&mut db)
        .await?;

    // Verify column list and that the discriminant is stored as I64, not a string or record
    assert_struct!(t.log().pop_op(), Operation::QuerySql(_ {
        stmt: Statement::Insert(_ {
            source.body: ExprSet::Values(_ {
                rows: [== (Any, Any, 1i64)],
                ..
            }),
            target: toasty_core::stmt::InsertTarget::Table(_ {
                table: == user_table,
                columns: == columns(&db, "users", &["id", "name", "status"]),
                ..
            }),
            ..
        }),
        ..
    }));

    // Read: discriminant is loaded back and converted to the enum variant
    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(found.status, Status::Pending);

    // Update (instance): replace the enum variant
    t.log().clear();
    user.update().status(Status::Active).exec(&mut db).await?;

    // Verify the status column receives the new discriminant as I64
    // Column index 2 is "status"; value I64(2) = Active discriminant
    if t.capability().sql {
        assert_struct!(t.log().pop_op(), Operation::QuerySql(_ {
            stmt: Statement::Update(_ {
                target: toasty_core::stmt::UpdateTarget::Table(== user_table),
                assignments: #{ 2: _ { expr: == 2i64, .. }},
                ..
            }),
            ..
        }));
    } else {
        assert_struct!(t.log().pop_op(), Operation::UpdateByKey(_ {
            table: == user_table,
            filter: None,
            keys: _,
            assignments: #{ 2: _ { expr: == 2i64, .. }},
            returning: false,
            ..
        }));
    }

    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(found.status, Status::Active);

    // Update (query-based): same replacement via filter builder
    User::filter_by_id(user.id)
        .update()
        .status(Status::Done)
        .exec(&mut db)
        .await?;

    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(found.status, Status::Done);

    // Delete: cleanup
    let id = user.id;
    user.delete(&mut db).await?;
    assert_err!(User::get_by_id(&mut db, &id).await);
    Ok(())
}

/// Tests filtering records by embedded enum variant.
/// SQL-only: DynamoDB requires a partition key in queries.
/// Validates that enum fields can be used in WHERE clauses (comparing discriminants),
/// and verifies the driver-level representation: the WHERE clause compares the status
/// column to an I64 discriminant, not a string or other type.
#[driver_test(requires(sql))]
pub async fn filter_by_enum_variant(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Status {
        #[column(variant = 1)]
        Pending,
        #[column(variant = 2)]
        Active,
        #[column(variant = 3)]
        Done,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Task {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        status: Status,
    }

    let mut db = t.setup_db(models!(Task, Status)).await;

    // Create tasks with different statuses: 1 pending, 2 active, 1 done
    for (name, status) in [
        ("Task A", Status::Pending),
        ("Task B", Status::Active),
        ("Task C", Status::Active),
        ("Task D", Status::Done),
    ] {
        Task::create()
            .name(name)
            .status(status)
            .exec(&mut db)
            .await?;
    }

    let status_col = column(&db, "tasks", "status");
    t.log().clear();

    // Filter: only Active tasks (discriminant = 2)
    let active = Task::filter(Task::fields().status().eq(Status::Active))
        .collect::<Vec<_>>(&mut db)
        .await?;
    assert_eq!(active.len(), 2);
    {
        let (op, _) = t.log().pop();
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Query(_ {
                body: ExprSet::Select(_ {
                    filter.expr: Some(Expr::BinaryOp(_ {
                        lhs.as_expr_column_unwrap().column: == status_col.index,
                        op: BinaryOp::Eq,
                        *rhs: == 2i64,
                        ..
                    })),
                    ..
                }),
                ..
            }),
            ..
        }));
    }

    // Filter: only Pending tasks (discriminant = 1)
    let pending = Task::filter(Task::fields().status().eq(Status::Pending))
        .collect::<Vec<_>>(&mut db)
        .await?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].name, "Task A");
    {
        let (op, _) = t.log().pop();
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Query(_ {
                body: ExprSet::Select(_ {
                    filter.expr: Some(Expr::BinaryOp(_ {
                        lhs.as_expr_column_unwrap().column: == status_col.index,
                        op: BinaryOp::Eq,
                        *rhs: == 1i64,
                        ..
                    })),
                    ..
                }),
                ..
            }),
            ..
        }));
    }

    // Filter: only Done tasks (discriminant = 3)
    let done = Task::filter(Task::fields().status().eq(Status::Done))
        .collect::<Vec<_>>(&mut db)
        .await?;
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].name, "Task D");
    {
        let (op, _) = t.log().pop();
        assert_struct!(op, Operation::QuerySql(_ {
            stmt: Statement::Query(_ {
                body: ExprSet::Select(_ {
                    filter.expr: Some(Expr::BinaryOp(_ {
                        lhs.as_expr_column_unwrap().column: == status_col.index,
                        op: BinaryOp::Eq,
                        *rhs: == 3i64,
                        ..
                    })),
                    ..
                }),
                ..
            }),
            ..
        }));
    }

    Ok(())
}

/// Tests that embedded enums are registered in the app schema but don't create
/// their own database tables (they're inlined into parent models as a single column).
#[driver_test]
pub async fn basic_embedded_enum(test: &mut Test) {
    #[derive(toasty::Embed)]
    enum Status {
        #[column(variant = 1)]
        Pending,
        #[column(variant = 2)]
        Active,
        #[column(variant = 3)]
        Done,
    }

    let db = test.setup_db(models!(Status)).await;
    let schema = db.schema();

    // Embedded enums exist in app schema as Model::EmbeddedEnum
    assert_struct!(schema.app.models, #{
        Status::id(): toasty::schema::app::Model::EmbeddedEnum(_ {
            name.upper_camel_case(): "Status",
            variants: [
                _ { name.upper_camel_case(): "Pending", discriminant: 1, .. },
                _ { name.upper_camel_case(): "Active", discriminant: 2, .. },
                _ { name.upper_camel_case(): "Done", discriminant: 3, .. },
            ],
            ..
        }),
    });

    // Embedded enums don't create database tables (stored as a column in parent)
    assert!(schema.db.tables.is_empty());
}

/// Tests the complete schema generation and mapping for an embedded enum field:
/// - App schema: enum field with correct type reference
/// - DB schema: enum field stored as a single INTEGER column
/// - Mapping: enum field maps directly to a primitive column (discriminant IS the value)
#[driver_test]
pub async fn root_model_with_embedded_enum_field(test: &mut Test) {
    #[derive(toasty::Embed)]
    enum Status {
        #[column(variant = 1)]
        Pending,
        #[column(variant = 2)]
        Active,
        #[column(variant = 3)]
        Done,
    }

    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        #[allow(dead_code)]
        status: Status,
    }

    let db = test.setup_db(models!(User, Status)).await;
    let schema = db.schema();

    // Both embedded enum and root model exist in app schema
    assert_struct!(schema.app.models, #{
        Status::id(): toasty::schema::app::Model::EmbeddedEnum(_ {
            name.upper_camel_case(): "Status",
            variants.len(): 3,
            ..
        }),
        User::id(): toasty::schema::app::Model::Root(_ {
            name.upper_camel_case(): "User",
            fields: [
                _ { name.app_name: "id", .. },
                _ {
                    name.app_name: "status",
                    ty: FieldTy::Embedded(_ {
                        target: == Status::id(),
                        ..
                    }),
                    ..
                }
            ],
            ..
        }),
    });

    // Database table has a single INTEGER column for the enum discriminant
    assert_struct!(schema.db.tables, [
        _ {
            name: =~ r"users$",
            columns: [
                _ { name: "id", .. },
                _ { name: "status", .. },
            ],
            ..
        }
    ]);

    let user = &schema.app.models[&User::id()];
    let user_table = schema.table_for(user);
    let user_mapping = &schema.mapping.models[&User::id()];

    assert_struct!(user_mapping, _ {
        columns.len(): 2,
        fields: [
            mapping::Field::Primitive(FieldPrimitive {
                column: == user_table.columns[0].id,
                lowering: 0,
                ..
            }),
            mapping::Field::Enum(FieldEnum {
                discriminant: FieldPrimitive {
                    column: == user_table.columns[1].id,
                    lowering: 1,
                    ..
                },
                variants.len(): 3,
                ..
            }),
        ],
        ..
    });
}
