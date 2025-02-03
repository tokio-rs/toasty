use super::*;

impl Verify<'_> {
    // Iterate each model and make sure there is an index path that enables
    // querying
    pub(super) fn verify_relations_are_indexed(&self, field: &Field) {
        use app::FieldTy;

        match &field.ty {
            FieldTy::BelongsTo(rel) => self.verify_belongs_to_is_indexed(rel),
            FieldTy::HasMany(rel) => self.verify_has_many_relation_is_indexed(rel),
            FieldTy::HasOne(rel) => self.verify_has_one_relation_is_indexed(rel),
            _ => {}
        }
    }

    fn verify_belongs_to_is_indexed(&self, _: &app::BelongsTo) {
        // TODO: Is there any necessary verification here?
    }

    fn verify_has_many_relation_is_indexed(&self, rel: &app::HasMany) {
        self.verify_has_relation_is_indexed(rel.target(&self.schema.app), rel.pair);
    }

    fn verify_has_one_relation_is_indexed(&self, rel: &app::HasOne) {
        self.verify_has_relation_is_indexed(rel.target(&self.schema.app), rel.pair);
    }

    fn verify_has_relation_is_indexed(&self, target: &Model, pair: FieldId) {
        let belongs_to = self.schema.app.field(pair).ty.expect_belongs_to();

        // Find an index that starts with the relations pair field and either
        // has no more fields or the next field is of local scope. This ensures
        // the ability to query all associated models.
        'outer: for index in &target.indices {
            assert!(!index.fields.is_empty());

            if index.fields.len() < belongs_to.foreign_key.fields.len() {
                continue;
            }

            for (i, fk_field) in belongs_to.foreign_key.fields.iter().enumerate() {
                if index.fields[i].field != fk_field.source {
                    continue 'outer;
                }
            }

            // The first index field matches the foreign key. If there are no
            // more index fields, then the index is an exact match.
            if index.fields.len() == belongs_to.foreign_key.fields.len() {
                return;
            }

            // If the next field is of local scope, then the index can be used.
            if index.fields[belongs_to.foreign_key.fields.len()]
                .scope
                .is_local()
            {
                return;
            }

            // The index is not a match
        }

        panic!("failed to find relation index");
    }
}
