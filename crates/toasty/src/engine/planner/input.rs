use super::*;

// fn partition_expr_input(expr: &mut stmt::Expr, sources: &[plan::InputSource])

struct Partitioner<'a> {
    planner: &'a Planner<'a>,
    sources: &'a [plan::InputSource],
    input: Option<plan::Input>,
}

enum Partition {
    Stmt,
    Eval {
        source: plan::InputSource,
        project: stmt::Expr,
    },
}

impl Partition {
    fn is_stmt(&self) -> bool {
        matches!(self, Self::Stmt)
    }
}

impl Partitioner<'_> {
    fn partition_expr(&mut self, expr: &mut stmt::Expr) -> Partition {
        match expr {
            stmt::Expr::Arg(expr) => Partition::Eval {
                source: self.sources[expr.position],
                project: stmt::Expr::arg(0),
            },
            stmt::Expr::Column(_) => Partition::Stmt,
            stmt::Expr::InList(expr) => {
                assert!(self.partition_expr(&mut expr.expr).is_stmt(), "TODO");

                if let Partition::Eval { source, project } = self.partition_expr(&mut expr.list) {
                    assert!(self.input.is_none());
                    let ty = self.planner.var_table.ty(&source).clone();

                    debug_assert!(ty.is_list(), "ty={ty:#?}");

                    self.input = Some(plan::Input {
                        source,
                        project: eval::Func::from_stmt(project, vec![ty]),
                    });
                    *expr.list = stmt::Expr::arg(0);
                }

                Partition::Stmt
            }
            stmt::Expr::Map(expr) => {
                let Partition::Eval { source, project } = self.partition_expr(&mut expr.base)
                else {
                    todo!()
                };

                assert!(project.is_arg());

                // For now, assume the mapping is fine w/o checking it Also,
                // this is a pretty mega hack since we are just removing the
                // map, assuming that this is the top-level projection.
                Partition::Eval {
                    source,
                    project: stmt::Expr::map(project, expr.map.take()),
                }
            }
            _ => todo!("{expr:#?}"),
        }
    }
}

impl Planner<'_> {
    pub(crate) fn partition_stmt_delete_input(
        &mut self,
        stmt: &mut stmt::Delete,
        sources: &[plan::InputSource],
    ) -> Option<plan::Input> {
        let mut partitioner = Partitioner {
            planner: &*self,
            sources,
            input: None,
        };

        let partition = partitioner.partition_expr(&mut stmt.filter);
        assert!(partition.is_stmt());

        partitioner.input
    }

    pub(crate) fn partition_stmt_query_input(
        &mut self,
        stmt: &mut stmt::Query,
        sources: &[plan::InputSource],
    ) -> Option<plan::Input> {
        assert!(sources.len() <= 1, "sources={sources:#?}");
        let mut partitioner = Partitioner {
            planner: &*self,
            sources,
            input: None,
        };

        match &mut stmt.body {
            stmt::ExprSet::Select(select) => {
                assert!(select.source.is_table());
                let partition = partitioner.partition_expr(&mut select.filter);
                assert!(partition.is_stmt());
            }
            _ => todo!("{stmt:#?}"),
        }

        partitioner.input
    }
}
