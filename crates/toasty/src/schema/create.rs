use crate::stmt::{IntoExpr, IntoInsert};

/// Trait for types that can produce a create builder for model `T`.
///
/// Both [`Model`](super::Model) and [`Relation`](super::Relation) extend this
/// trait so that generic code can obtain a builder without caring whether it is
/// dealing with a root model or a relation wrapper.
pub trait Create<T> {
    /// The builder type used to construct a new instance of `T`.
    type Builder: Default + IntoInsert<Model = T> + IntoExpr<T>;

    /// Return a fresh, default-initialized builder.
    fn builder() -> Self::Builder {
        Self::Builder::default()
    }
}
