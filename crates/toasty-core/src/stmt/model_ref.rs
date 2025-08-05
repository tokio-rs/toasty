use super::*;
use crate::schema::app::{ModelId, Schema};
use std::any::TypeId;

/// A reference to a model that can be either unresolved (TypeId) or resolved (ModelId).
///
/// This enum enables the public API to use TypeId references while the internal
/// engine uses resolved ModelId for efficient array-based access.
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
    pub fn resolve(&mut self, schema: &Schema) -> Result<()> {
        if let Self::Type(type_id) = *self {
            let model_id = schema
                .type_to_model_id(type_id)
                .map_err(|_| ModelRefError { type_id })?;
            *self = Self::Resolved(model_id);
        }
        Ok(())
    }

    /// Get the resolved ModelId, panicking if not resolved
    ///
    /// # Panics
    /// Panics if the ModelRef has not been resolved yet. Call `resolve()` first.
    pub fn model_id(&self) -> ModelId {
        match self {
            Self::Resolved(id) => *id,
            Self::Type(type_id) => panic!(
                "ModelRef not resolved - call resolve() first. TypeId: {:?}",
                type_id
            ),
        }
    }

    /// Try to get the resolved ModelId without panicking
    pub fn try_model_id(&self) -> Option<ModelId> {
        match self {
            Self::Resolved(id) => Some(*id),
            Self::Type(_) => None,
        }
    }

    /// Check if this ModelRef has been resolved
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

impl From<TypeId> for ModelRef {
    fn from(type_id: TypeId) -> Self {
        Self::Type(type_id)
    }
}

impl From<ModelId> for ModelRef {
    fn from(model_id: ModelId) -> Self {
        Self::Resolved(model_id)
    }
}

/// Error type for ModelRef resolution failures
#[derive(Debug)]
pub struct ModelRefError {
    pub type_id: TypeId,
}

impl std::fmt::Display for ModelRefError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TypeId {:?} not found in schema - model may not be registered",
            self.type_id
        )
    }
}

impl std::error::Error for ModelRefError {}
