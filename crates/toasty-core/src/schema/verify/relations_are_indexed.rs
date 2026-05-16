use super::Verify;
use crate::schema::app::{self, Field, FieldId, Model, ModelRoot};
use crate::{Error, Result};

/// Returns `true` if `model` declares a single-column non-primary-key index
/// covering exactly `field`.
fn has_single_field_index(model: &ModelRoot, field: FieldId) -> bool {
    model.indices.iter().any(|index| {
        !index.primary_key && index.fields.len() == 1 && index.fields[0].field == field
    })
}

impl Verify<'_> {
    // Iterate each model and make sure there is an index path that enables
    // querying
    pub(super) fn verify_relations_are_indexed(&self, owner: &Model, field: &Field) -> Result<()> {
        use app::FieldTy;

        match &field.ty {
            FieldTy::BelongsTo(rel) => self.verify_belongs_to_is_indexed(rel),
            FieldTy::HasMany(rel) => self.verify_has_many_relation_is_indexed(owner, field, rel),
            FieldTy::HasOne(rel) => self.verify_has_one_relation_is_indexed(owner, field, rel),
            _ => Ok(()),
        }
    }

    fn verify_belongs_to_is_indexed(&self, _: &app::BelongsTo) -> Result<()> {
        // TODO: Is there any necessary verification here?
        Ok(())
    }

    fn verify_has_many_relation_is_indexed(
        &self,
        owner: &Model,
        field: &Field,
        rel: &app::HasMany,
    ) -> Result<()> {
        self.verify_has_relation_is_indexed(owner, field, rel.target(&self.schema.app), rel.pair)
    }

    fn verify_has_one_relation_is_indexed(
        &self,
        owner: &Model,
        field: &Field,
        rel: &app::HasOne,
    ) -> Result<()> {
        self.verify_has_relation_is_indexed(owner, field, rel.target(&self.schema.app), rel.pair)
    }

    fn verify_has_relation_is_indexed(
        &self,
        owner: &Model,
        field: &Field,
        target: &Model,
        pair: FieldId,
    ) -> Result<()> {
        let belongs_to = self.schema.app.field(pair).ty.as_belongs_to_unwrap();
        let target_root = target.as_root_unwrap();

        // Find an index that starts with the relations pair field and either
        // has no more fields or the next field is of local scope. This ensures
        // the ability to query all associated models.
        'outer: for index in &target_root.indices {
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
                return Ok(());
            }

            // If the next field is of local scope, then the index can be used.
            if index.fields[belongs_to.foreign_key.fields.len()]
                .scope
                .is_local()
            {
                return Ok(());
            }

            // The index is not a match
        }

        Err(self.missing_relation_index_error(owner, field, target_root, pair, belongs_to))
    }

    /// Build a helpful `invalid_schema` error explaining that no covering
    /// index exists for `belongs_to` and suggesting how to add one.
    fn missing_relation_index_error(
        &self,
        owner: &Model,
        field: &Field,
        target_root: &ModelRoot,
        pair: FieldId,
        belongs_to: &app::BelongsTo,
    ) -> Error {
        let owner_name = owner.name();
        let target_name = &target_root.name;
        let rel_field = &field.name;
        let pair_field = &self.schema.app.field(pair).name;

        let fk_field_names = belongs_to
            .foreign_key
            .fields
            .iter()
            .map(|fk| self.schema.app.field(fk.source).name.to_string())
            .collect::<Vec<_>>();

        let hint = if fk_field_names.len() == 1 {
            format!(
                "add `#[index]` to field `{}` on model `{}`",
                fk_field_names[0], target_name,
            )
        } else if belongs_to
            .foreign_key
            .fields
            .iter()
            .all(|fk| has_single_field_index(target_root, fk.source))
        {
            // Each FK field has its own single-column index. Two single-column
            // indexes don't compose into a covering index for a composite
            // foreign key — the user must replace them with a composite index.
            format!(
                "each foreign-key field already has its own `#[index]`, but a \
                 composite foreign key needs a single covering index. Replace \
                 the per-field `#[index]` annotations with a model-level \
                 `#[index({})]` on `{}`",
                fk_field_names.join(", "),
                target_name,
            )
        } else {
            format!(
                "add `#[index({})]` to model `{}`",
                fk_field_names.join(", "),
                target_name,
            )
        };

        Error::invalid_schema(format!(
            "relation `{owner_name}::{rel_field}` cannot be queried: \
             no index on `{target_name}` covers the foreign key \
             declared by `{target_name}::{pair_field}` (fields: {}). \
             Hint: {hint}.",
            fk_field_names.join(", "),
        ))
    }
}
