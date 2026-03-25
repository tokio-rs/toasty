use crate::stmt::{IntoExpr, IntoInsert};

/// Trait for types that can produce a create builder.
///
/// Both [`Model`](super::Model) and [`Relation`](super::Relation) extend this
/// trait so that generic code can obtain a builder without caring whether it is
/// dealing with a root model or a relation wrapper.
pub trait Create {
    /// The model type that this builder creates.
    type Item;

    /// The builder type used to construct a new instance.
    type Builder: Default + IntoInsert<Model = Self::Item> + IntoExpr<Self::Item>;

    /// Return a fresh, default-initialized builder.
    fn builder() -> Self::Builder {
        Self::Builder::default()
    }
}
