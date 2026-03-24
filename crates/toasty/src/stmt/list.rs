use std::marker::PhantomData;

/// A sized marker type representing "list of `M`".
///
/// Used as a type parameter to [`Statement`], [`Load`](crate::schema::Load), and other
/// types to encode that the result is a collection of `M` values. Unlike `[M]`
/// (which is unsized), `List<M>` is always `Sized`, so it composes cleanly in
/// tuples: `(List<User>, List<Todo>)` is valid.
pub struct List<M>(PhantomData<M>);
