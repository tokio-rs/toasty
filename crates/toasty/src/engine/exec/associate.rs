use super::{plan, Exec, Result};
use toasty_core::stmt::ValueStream;
use toasty_core::{schema::app::FieldTy, stmt};

impl Exec<'_> {
    pub(super) async fn action_associate(&mut self, action: &plan::Associate) -> Result<()> {
        let mut source = self.vars.load(action.source).collect().await?;
        let target = self.vars.load(action.target).collect().await?;

        match &self.db.schema.app.field(action.field).ty {
            FieldTy::BelongsTo(rel) => {
                for source_item in &mut source {
                    let source_item = source_item.expect_record_mut();

                    for target_item in &target {
                        let target_item = target_item.expect_record();

                        let [fk_field] = &rel.foreign_key.fields[..] else {
                            todo!("composite keys")
                        };

                        if source_item[fk_field.source.index] == target_item[fk_field.target.index]
                        {
                            source_item[action.field.index] = target_item.clone().into();
                            break;
                        }
                    }
                }
            }
            FieldTy::HasMany(rel) => {
                let pair = rel.pair(&self.db.schema.app);

                let [fk_field] = &pair.foreign_key.fields[..] else {
                    todo!("composite keys")
                };

                let mut target_by_fk: std::collections::HashMap<stmt::Value, Vec<stmt::ValueRecord>> =
                    std::collections::HashMap::new();

                for target_item in &target {
                    let target_item = target_item.expect_record();
                    let fk_value = target_item[fk_field.source.index].clone();
                    target_by_fk
                        .entry(fk_value)
                        .or_default()
                        .push(target_item.clone());
                }

                for source_item in &mut source {
                    let source_item = source_item.expect_record_mut();
                    let pk_value = &source_item[fk_field.target.index];

                    let associated = target_by_fk
                        .get(pk_value)
                        .map(|records| records.iter().map(|r| r.clone().into()).collect())
                        .unwrap_or_default();

                    source_item[action.field.index] = stmt::Value::List(associated);
                }
            }
            FieldTy::HasOne(rel) => {
                let pair = rel.pair(&self.db.schema.app);

                for source_item in &mut source {
                    let source_item = source_item.expect_record_mut();

                    for target_item in &target {
                        let target_item = target_item.expect_record();

                        let [fk_field] = &pair.foreign_key.fields[..] else {
                            todo!("composite keys")
                        };

                        if target_item[fk_field.source.index] == source_item[fk_field.target.index]
                        {
                            source_item[action.field.index] = target_item.clone().into();
                            break;
                        }
                    }
                }
            }
            _ => todo!(),
        }

        self.vars
            .store(action.source, ValueStream::from_vec(source));
        Ok(())
    }
}
