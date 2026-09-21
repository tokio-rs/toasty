use crate::schema::app::{FieldId, ModelId, Schema, VariantId};
use crate::stmt;

/// The location of a belongs-to relation on a host model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    /// Embed fields descended through, outermost first.
    pub steps: Vec<PairStep>,
    /// The relation field on the model or final embedded type.
    pub field: FieldId,
}

/// One descent through an embedded field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairStep {
    /// The embedded field.
    pub field: FieldId,
    /// The selected variant, or `None` for a struct.
    pub variant: Option<VariantId>,
}

/// A belongs-to definition instantiated on a particular host model.
#[derive(Debug, Clone)]
pub struct RelationInstance {
    /// The location of the belongs-to field.
    pub location: Pair,
    /// The inverse relation declared for this embedding.
    pub pair: Option<FieldId>,
}

impl Pair {
    /// Locates a relation directly on a model.
    pub fn direct(field: FieldId) -> Self {
        Self {
            steps: vec![],
            field,
        }
    }

    /// The root model containing this relation instance.
    pub fn host(&self) -> ModelId {
        self.steps
            .first()
            .map_or(self.field.model, |step| step.field.model)
    }

    /// Builds a path to the relation, preserving variant selections.
    pub fn path(&self, schema: &Schema) -> stmt::Path {
        self.field_path(schema, self.field)
    }

    /// Builds a path to a sibling of the relation, such as a foreign key.
    pub fn field_path(&self, schema: &Schema, field: FieldId) -> stmt::Path {
        let mut path = stmt::Path::model(self.host());
        let mut variant = None;
        for (field, selected) in self
            .steps
            .iter()
            .map(|step| (step.field, step.variant))
            .chain(std::iter::once((field, None)))
        {
            let index = match variant {
                Some(id) => schema
                    .model(field.model)
                    .as_embedded_enum_unwrap()
                    .variant_fields(id)
                    .iter()
                    .position(|f| f.id == field)
                    .unwrap(),
                None => field.index,
            };
            path.projection.push(index);
            if let Some(id) = selected {
                path = stmt::Path::from_variant(path, id);
            }
            variant = selected.map(|id| id.index);
        }
        path
    }
}
