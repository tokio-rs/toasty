use super::{plan, Exec, Result};
use std::collections::HashMap;
use toasty_core::stmt::ValueStream;
use toasty_core::{schema::app::FieldTy, stmt};

impl Exec<'_> {
    pub(super) async fn action_associate(&mut self, action: &plan::Associate) -> Result<()> {
        let mut source = self.vars.load(action.source).collect().await?;
        let target = self.vars.load(action.target).collect().await?;

        match &self.engine.schema.app.field(action.field).ty {
            FieldTy::BelongsTo(rel) => {
                let [fk_field] = &rel.foreign_key.fields[..] else {
                    todo!("composite keys")
                };

                // Index source items: HashMap<Value, &mut Record>
                let mut source_by_fk: HashMap<stmt::Value, &mut stmt::ValueRecord> = HashMap::new();
                for source_item in &mut source {
                    let source_record = source_item.expect_record_mut();
                    let fk_value = source_record[fk_field.source.index].clone();
                    source_by_fk.insert(fk_value, source_record);
                }

                for target_item in &target {
                    let target_record = target_item.expect_record();
                    let pk_value = &target_record[fk_field.target.index];

                    if let Some(source_record) = source_by_fk.get_mut(pk_value) {
                        source_record[action.field.index] = target_record.clone().into();
                    }
                }
            }
            FieldTy::HasMany(rel) => {
                let pair = rel.pair(&self.engine.schema.app);

                let [fk_field] = &pair.foreign_key.fields[..] else {
                    todo!("composite keys")
                };

                let mut source_by_pk: HashMap<stmt::Value, &mut stmt::ValueRecord> = HashMap::new();
                for source_item in &mut source {
                    let source_record = source_item.expect_record_mut();
                    let pk_value = source_record[fk_field.target.index].clone();
                    source_by_pk.insert(pk_value, source_record);
                }

                for target_item in &target {
                    let target_record = target_item.expect_record();
                    let fk_value = &target_record[fk_field.source.index];

                    if let Some(source_record) = source_by_pk.get_mut(fk_value) {
                        if !matches!(source_record[action.field.index], stmt::Value::List(_)) {
                            source_record[action.field.index] = stmt::Value::List(Vec::new());
                        }
                        if let stmt::Value::List(ref mut list) = source_record[action.field.index] {
                            list.push(target_record.clone().into());
                        }
                    }
                }
            }
            FieldTy::HasOne(rel) => {
                let pair = rel.pair(&self.engine.schema.app);

                let [fk_field] = &pair.foreign_key.fields[..] else {
                    todo!("composite keys")
                };

                let mut source_by_pk: HashMap<stmt::Value, &mut stmt::ValueRecord> = HashMap::new();
                for source_item in &mut source {
                    let source_record = source_item.expect_record_mut();
                    let pk_value = source_record[fk_field.target.index].clone();
                    source_by_pk.insert(pk_value, source_record);
                }

                for target_item in &target {
                    let target_record = target_item.expect_record();
                    let fk_value = &target_record[fk_field.source.index];

                    if let Some(source_record) = source_by_pk.get_mut(fk_value) {
                        source_record[action.field.index] = target_record.clone().into();
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
