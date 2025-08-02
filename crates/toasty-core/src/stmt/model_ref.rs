use super::*;
use std::any::TypeId;

/// Reference to a model that can be either unresolved (TypeId) or resolved (ModelId)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelRef {
    /// Unresolved type reference (used in public API)
    Type(TypeId),
    /// Resolved model ID (used internally after resolution)
    Resolved(ModelId),
}

impl ModelRef {
    /// Create a ModelRef from a type
    pub fn from_type<T: 'static>() -> Self {
        Self::Type(TypeId::of::<T>())
    }

    /// Create a ModelRef from a resolved ModelId
    pub fn from_model_id(id: ModelId) -> Self {
        Self::Resolved(id)
    }

    /// Resolve this ModelRef using the provided schema
    pub fn resolve(&mut self, schema: &crate::schema::Schema) -> crate::Result<()> {
        if let Self::Type(type_id) = *self {
            let model_id = schema.type_to_model_id(type_id)?;
            *self = Self::Resolved(model_id);
        }
        Ok(())
    }

    /// Get the ModelId, panicking if not resolved
    pub fn model_id(&self) -> ModelId {
        match self {
            Self::Resolved(id) => *id,
            Self::Type(_) => panic!("ModelRef not resolved - call resolve() first"),
        }
    }

    /// Try to get the ModelId, returning None if not resolved
    pub fn try_model_id(&self) -> Option<ModelId> {
        match self {
            Self::Resolved(id) => Some(*id),
            Self::Type(_) => None,
        }
    }

    /// Check if this ModelRef is resolved
    pub fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved(_))
    }

    /// Get the TypeId if this is an unresolved reference
    pub fn type_id(&self) -> Option<TypeId> {
        match self {
            Self::Type(type_id) => Some(*type_id),
            Self::Resolved(_) => None,
        }
    }
}

impl From<ModelId> for ModelRef {
    fn from(id: ModelId) -> Self {
        Self::Resolved(id)
    }
}

impl From<TypeId> for ModelRef {
    fn from(type_id: TypeId) -> Self {
        Self::Type(type_id)
    }
}
