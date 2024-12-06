use super::*;

// fn partition_expr_input(expr: &mut stmt::Expr, sources: &[plan::InputSource])

struct Partitioner<'a> {
    sources: &'a [plan::InputSource],
    input: Vec<plan::Input>,
}

enum Partition {
    Stmt,
    Eval {
        source: plan::InputSource,
        project: Option<eval::Expr>,
    },
}

impl Partition {
    fn is_stmt(&self) -> bool {
        matches!(self, Partition::Stmt)
    }
}

impl Partitioner<'_> {
    fn partition_expr(&mut self, expr: &mut stmt::Expr) -> Partition {
        match expr {
            stmt::Expr::Arg(expr) => Partition::Eval {
                source: self.sources[expr.position].clone(),
                project: None,
            },
            stmt::Expr::Column(_) => Partition::Stmt,
            stmt::Expr::InList(expr) => {
                assert!(self.partition_expr(&mut *expr.expr).is_stmt(), "TODO");

                if let Partition::Eval { source, project } = self.partition_expr(&mut *expr.list) {
                    let position = self.input.len();
                    self.input.push(plan::Input { source, project });
                    *expr.list = stmt::Expr::arg(position);
                }

                Partition::Stmt
            }
            stmt::Expr::Map(expr) => {
                let Partition::Eval { source, project } = self.partition_expr(&mut *expr.base)
                else {
                    todo!()
                };

                assert!(project.is_none());

                // For now, assume the mapping is fine w/o checking it Also,
                // this is a pretty mega hack since we are just removing the
                // map, assuming that this is the top-level projection.
                Partition::Eval {
                    source,
                    project: Some(eval::Expr::try_from_stmt(expr.map.take(), ()).unwrap()),
                }
            }
            _ => todo!("{expr:#?}"),
        }
    }
}

impl Planner<'_> {
    pub(crate) fn partition_query_input(
        &mut self,
        stmt: &mut stmt::Query,
        sources: &[plan::InputSource],
    ) -> Vec<plan::Input> {
        let mut partitioner = Partitioner {
            sources,
            input: vec![],
        };

        match &mut *stmt.body {
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
