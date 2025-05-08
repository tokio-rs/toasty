use super::*;

use app::{FieldTy, Model};

// Strategy:
// * Create a batch of queries to operate atomically.
// * Queries might mix `insert`, `update`, and `delete`
// * Since Update may insert, it could trigger the full insertion planning path.

impl Planner<'_> {
    // If the update statement requested the result to be returned, then this
    // method returns the var in which it will be stored.
    pub(super) fn plan_stmt_update(
        &mut self,
        mut stmt: stmt::Update,
    ) -> Result<Option<plan::VarId>> {
        let model = self.model(stmt.target.as_model_id());

        // Make sure the update statement isn't empty
        assert!(
            !stmt.assignments.is_empty(),
            "update must update some columns"
        );

        let scope = stmt.selection();

        // Handle any relation updates
        for (i, field) in model.fields.iter().enumerate() {
            if !stmt.assignments.contains(i) {
                continue;
            }

            let Some(assignment) = stmt.assignments.get_mut(&i) else {
                continue;
            };

            match assignment.op {
                stmt::AssignmentOp::Set => assert!(!field.ty.is_has_many(), "TODO"),
                stmt::AssignmentOp::Insert => assert!(field.ty.is_has_many(), "TODO"),
                stmt::AssignmentOp::Remove => assert!(field.ty.is_has_many(), "TODO"),
            }

            self.plan_mut_relation_field(
                field,
                assignment.op,
                &mut assignment.expr,
                &scope,
                false,
            )?;

            // Map the belongs_to statement to the foreign key fields
            if let FieldTy::BelongsTo(belongs_to) = &field.ty {
                let stmt::Expr::Value(value) = stmt.assignments.take(i).expr else {
                    todo!()
                };

                match value {
                    stmt::Value::Null => {
                        for fk_field in &belongs_to.foreign_key.fields {
                            stmt.assignments.set(fk_field.source, stmt::Expr::null());
                        }
                    }
                    value => {
                        let [fk_field] = &belongs_to.foreign_key.fields[..] else {
                            todo!("composite keys")
                        };

                        stmt.assignments.set(fk_field.source, value);
                    }
                }
            } else if field.is_relation() {
                stmt.assignments.unset(i);
            }
        }

        if stmt.assignments.is_empty() {
            if stmt.returning.is_none() {
                return Ok(None);
            }

            let value = stmt::Value::empty_sparse_record();
            return Ok(Some(self.set_var(
                vec![value],
                stmt::Type::list(stmt::Type::empty_sparse_record()),
            )));
        }

        if !self.capability.sql {
            // Subqueries are planned before lowering
            self.plan_subqueries(&mut stmt)?;
        }

        self.lower_stmt_update(model, &mut stmt);

        // If the statement `Returning` is constant (i.e. does not depend on the
        // database evaluating the statement), then extract it here.
        self.constantize_update_returning(&mut stmt);

        let output = self
            .partition_maybe_returning(&mut stmt.returning)
            .map(|project| plan::Output {
                var: self
                    .var_table
                    .register_var(stmt::Type::list(project.ret.clone())),
                project,
            });

        Ok(if self.capability.sql {
            self.plan_update_sql(stmt, output)
        } else {
            self.plan_update_kv(model, stmt, output)
        })
    }

    fn plan_update_sql(
        &mut self,
        stmt: stmt::Update,
        output: Option<plan::Output>,
    ) -> Option<plan::VarId> {
        let output_var = output.as_ref().map(|o| o.var);

        // SQL does not support update conditions, so we need to rewrite the
        // statement. This is a bit tricky because the best strategy for
        // rewriting the statement will depend on the target database.
        if stmt.condition.is_some() {
            if self.capability.cte_with_update {
                let stmt = self.rewrite_conditional_update_as_query_with_cte(stmt);

                assert!(output.is_none());

                self.push_action(plan::ExecStatement {
                    output,
                    input: None,
                    stmt: stmt.into(),
                    conditional_update_with_no_returning: true,
                });
            } else {
                self.plan_conditional_update_as_transaction(stmt, output);
            }
        } else {
            // SQLite does not support CTE with update. We should transform the
            // conditional update into a transaction with checks between.
            // However, for now, the SQLite driver handles it by hand (kind of).
            self.push_action(plan::ExecStatement {
                output,
                input: None,
                stmt: stmt.into(),
                conditional_update_with_no_returning: false,
            });
        }

        output_var
    }

    fn plan_update_kv(
        &mut self,
        model: &Model,
        stmt: stmt::Update,
        output: Option<plan::Output>,
    ) -> Option<plan::VarId> {
        let table = self.schema.table_for(model);

        let mut index_plan =
            self.plan_index_path2(table, stmt.filter.as_ref().expect("no filter specified"));

        assert!(!stmt.assignments.is_empty());

        let output_var = output.as_ref().map(|o| o.var);

        if index_plan.index.primary_key {
            let Some(key) = self.try_build_key_filter(index_plan.index, &index_plan.index_filter)
            else {
                todo!("index_filter={:#?}", index_plan.index_filter);
            };

            self.push_write_action(plan::UpdateByKey {
                input: None,
                output,
                table: table.id,
                keys: key,
                assignments: stmt.assignments,
                filter: index_plan.result_filter,
                condition: stmt.condition,
            });

            output_var
        } else {
            debug_assert!(index_plan.post_filter.is_none());
            debug_assert!(!stmt.assignments.is_empty());
            assert!(stmt.condition.is_none());

            // Find existing associations so we can delete them
            // TODO: this should be atomic
            let update_by_key_input = self.plan_find_pk_by_index(&mut index_plan, None);
            let keys = eval::Func::identity(update_by_key_input.project.ret.clone());

            self.push_write_action(plan::UpdateByKey {
                input: Some(update_by_key_input),
                output,
                table: table.id,
                keys,
                assignments: stmt.assignments,
                filter: index_plan.result_filter,
                condition: None,
            });

            output_var
        }
    }

    // Constantizing the returning clause of an update statement is a bit tricky as there could be many rows that are impacted.
    fn constantize_update_returning(&self, stmt: &mut stmt::Update) {
        let Some(stmt::Returning::Expr(returning)) = &mut stmt.returning else {
            return;
        };

        struct ConstReturning<'a> {
            assignments: &'a stmt::Assignments,
        }

        impl stmt::visit_mut::VisitMut for ConstReturning<'_> {
            fn visit_expr_mut(&mut self, expr: &mut stmt::Expr) {
                stmt::visit_mut::visit_expr_mut(self, expr);

                if let stmt::Expr::Column(stmt::ExprColumn::Column(column_id)) = expr {
                    if let Some(assignment) = self.assignments.get(&column_id.index) {
                        assert!(assignment.op.is_set());
                        assert!(assignment.expr.is_const());

                        *expr = assignment.expr.clone();
                    }
                }
            }
        }

        ConstReturning {
            assignments: &stmt.assignments,
        }
        .visit_expr_mut(returning);
    }

    fn rewrite_conditional_update_as_query_with_cte(&self, stmt: stmt::Update) -> stmt::Query {
        let Some(condition) = stmt.condition else {
            panic!("conditional update without condition");
        };

        let Some(filter) = stmt.filter else {
            panic!("conditional update without filter");
        };

        let stmt::UpdateTarget::Table(target) = stmt.target.clone() else {
            panic!("conditional update without table");
        };

        let mut ctes = vec![];

        // Select from update table without the update condition.
        ctes.push(stmt::Cte {
            query: stmt::Query::builder(target)
                .filter(filter.clone())
                .returning(vec![
                    stmt::Expr::count_star(),
                    stmt::FuncCount {
                        arg: None,
                        filter: Some(Box::new(condition)),
                    }
                    .into(),
                ])
                .build(),
        });

        let returning_len = match &stmt.returning {
            Some(stmt::Returning::Expr(expr)) => {
                let stmt::Expr::Record(expr_record) = expr else {
                    panic!("returning must be a record");
                };

                expr_record.fields.len()
            }
            Some(_) => todo!(),
            None => 0,
        };

        // The update statement. The update condition is expressed using the select above
        ctes.push(stmt::Cte {
            query: stmt::Query::new(stmt::Update {
                target: stmt.target,
                assignments: stmt.assignments,
                filter: Some(stmt::Expr::and(
                    filter,
                    // SELECT found.count(*) = found.count(CONDITION) FROM found
                    stmt::Expr::stmt(stmt::Select {
                        source: stmt::TableRef::Cte {
                            nesting: 2,
                            index: 0,
                        }
                        .into(),
                        filter: true.into(),
                        returning: stmt::Returning::Expr(stmt::Expr::eq(
                            stmt::ExprColumn::Alias {
                                nesting: 0,
                                table: 0,
                                column: 0,
                            },
                            stmt::ExprColumn::Alias {
                                nesting: 0,
                                table: 0,
                                column: 1,
                            },
                        )),
                    }),
                )),
                condition: None,
                returning: Some(
                    stmt.returning
                        // TODO: hax
                        .unwrap_or_else(|| stmt::Returning::Expr(stmt::Expr::from("hello"))),
                ),
            }),
        });

        let mut columns = vec![
            stmt::ExprColumn::Alias {
                nesting: 0,
                table: 0,
                column: 0,
            }
            .into(),
            stmt::ExprColumn::Alias {
                nesting: 0,
                table: 0,
                column: 1,
            }
            .into(),
        ];

        for i in 0..returning_len {
            columns.push(
                stmt::ExprColumn::Alias {
                    nesting: 0,
                    table: 1,
                    column: i,
                }
                .into(),
            );
        }

        stmt::Query::builder(stmt::Select {
            source: stmt::Source::Table(vec![stmt::TableWithJoins {
                table: stmt::TableRef::Cte {
                    nesting: 0,
                    index: 0,
                },
                joins: vec![stmt::Join {
                    table: stmt::TableRef::Cte {
                        nesting: 0,
                        index: 1,
                    },
                    constraint: stmt::JoinOp::Left(stmt::Expr::from(true)),
                }],
            }]),
            filter: stmt::Expr::from(true),
            returning: stmt::Returning::Expr(stmt::Expr::record_from_vec(columns)),
        })
        .with(ctes)
        .build()
    }

    fn plan_conditional_update_as_transaction(
        &mut self,
        stmt: stmt::Update,
        output: Option<plan::Output>,
    ) {
        // For now, no returning supported
        assert!(stmt.returning.is_none(), "TODO: support returning");

        let Some(condition) = stmt.condition else {
            panic!("conditional update without condition");
        };

        let Some(filter) = stmt.filter else {
            panic!("conditional update without filter");
        };

        let stmt::UpdateTarget::Table(target) = stmt.target.clone() else {
            panic!("conditional update without table");
        };

        // Neither SQLite nor MySQL support CTE with update. We should transform
        // the conditional update into a transaction with checks between.

        /*
         * BEGIN;
         *
         * SELECT FOR UPDATE count(*), count({condition}) FROM {table} WHERE {filter};
         *
         * UPDATE {table} SET {assignments} WHERE {filter} AND @col_0 = @col_1;
         *
         * SELECT @col_0, @col_1;
         *
         * COMMIT;
         */

        let read = stmt::Query::builder(target)
            .filter(filter.clone())
            .returning(vec![
                stmt::Expr::count_star(),
                stmt::FuncCount {
                    arg: None,
                    filter: Some(Box::new(condition)),
                }
                .into(),
            ])
            .locks(if self.capability.select_for_update {
                vec![stmt::Lock::Update]
            } else {
                vec![]
            })
            .build();

        let write = stmt::Update {
            target: stmt.target,
            assignments: stmt.assignments,
            filter: Some(filter),
            condition: None,
            returning: None,
        };

        self.push_action(plan::ReadModifyWrite {
            input: None,
            output,
            read,
            write: write.into(),
        });
    }
}
