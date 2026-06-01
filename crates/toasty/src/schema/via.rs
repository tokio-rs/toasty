use super::{Deferred, Load};
use crate::stmt::{Association, List, Path, Query};

use toasty_core::schema::Name;
use toasty_core::schema::app::{self, FieldTy, ModelId, Via};
use toasty_core::stmt;

/// Placeholder via target for a scalar terminal, overwritten when `toasty-core`
/// resolves the relation chain while linking. It must never reach a query.
const UNRESOLVED_TARGET: ModelId = ModelId(usize::MAX);

/// Implemented by a `#[has_many(via = …)]` field's Rust type, exposing the
/// via's [`Target`](Self::Target) — the terminal type the path projects.
///
/// Blanket-implemented over `Vec<E>` and [`Deferred<Vec<E>>`](Deferred) for
/// every `E`, so the derive can route a via field through [`ViaTarget`]
/// without requiring `E: Model`. A single blanket impl per outer shape keeps
/// it coherent with the per-target `ViaTarget` impls.
pub trait ViaManyField {
    /// The via's terminal type — the model or scalar the path projects.
    type Target: ViaTarget;

    /// Whether the field stores its value in a deferred load slot.
    const DEFERRED: bool;
}

impl<E: ViaTarget> ViaManyField for Vec<E> {
    type Target = E;
    const DEFERRED: bool = false;
}

impl<E: ViaTarget> ViaManyField for Deferred<Vec<E>> {
    type Target = E;
    const DEFERRED: bool = true;
}

/// The query builder a `#[has_many(via = …)]` navigation method returns for a
/// field of type `F` (e.g. `Vec<String>`, `Deferred<Vec<Article>>`).
///
/// Resolves to the terminal type's [`ViaTarget::Query`]: `QueryMany<M>` for a
/// model terminal, `Query<List<scalar>>` for a scalar. Mirrors
/// [`QueryMany`](super::QueryMany), which aliases a model's query builder the
/// same way.
pub type ViaMany<F> = <<F as ViaManyField>::Target as ViaTarget>::Query;

/// A type that can be the terminal element of a `#[has_many(via = …)]`
/// relation.
///
/// A `via` relation reaches its terminal by following a path of existing
/// relations. The terminal is usually another model (`via = comments.article`),
/// but it may also be a **scalar field** (`via = todos.tags.name`), which
/// projects that field across the relation path to yield `Vec<String>`.
///
/// This trait is keyed on the *terminal element type*, with disjoint concrete
/// impls so they never overlap:
///
/// - The `#[derive(Model)]` macro emits an impl for each model, setting
///   [`Query`](Self::Query) to that model's generated query builder (so a
///   relation-terminal via keeps its rich `QueryMany<M>` API).
/// - This module emits an impl for each scalar primitive, setting
///   [`Query`](Self::Query) to the plain [`Query<List<Self>>`].
pub trait ViaTarget {
    /// The query builder returned by the generated has-many-via navigation
    /// method.
    type Query;

    /// The typed path handle a `#[has_many(via = …)]` field's accessor returns,
    /// parameterized by the origin model. Mirrors [`Model::Path`] for the
    /// via-many case.
    ///
    /// A **model** terminal returns that model's chainable
    /// [`ManyField`](crate::schema::Model::ManyField) — a `Path` wrapper that
    /// adds field accessors, so navigation can continue *through* a via step
    /// (e.g. `organizations.todos.title`, where `organizations.todos` is itself
    /// a via). A **scalar** terminal returns a plain list [`Path`]: a leaf with
    /// nothing further to chain.
    ///
    /// [`Model::Path`]: crate::schema::Model::Path
    type Path<Origin>;

    /// Build the [`FieldTy::Via`] for a `#[has_many(via = …)]` field whose
    /// terminal element type is `Self`.
    ///
    /// `path` is the fully-resolved field path (rooted at the declaring
    /// model), including the terminal step. The via target is `Self::id()` for
    /// a model terminal; for a scalar terminal it is the model the relation
    /// chain reaches, which the path alone can't name here — so the scalar
    /// impls leave it unset and `toasty-core` fills it in while linking.
    fn via_field_ty(singular: Name, path: stmt::Path) -> FieldTy;

    /// Build the [`Path`](Self::Path) handle from the typed `path` reaching this
    /// terminal. A model terminal wraps the path in its chainable `ManyField`; a
    /// scalar terminal is a leaf, so the path is the handle.
    fn new_path<Origin>(path: Path<Origin, List<Self>>) -> Self::Path<Origin>
    where
        Self: Sized;

    /// Wrap a via-field association into the navigation query.
    ///
    /// A scalar terminal yields a query whose source model and terminal
    /// projection are filled in during lowering ([`RewriteVia`]) from the
    /// core-resolved schema, so neither is needed here.
    ///
    /// [`RewriteVia`]: ../../engine/lower/association/struct.RewriteVia.html
    fn make_via_query(assoc: Association<List<Self>>) -> Self::Query
    where
        Self: Sized;
}

/// Emit `ViaTarget` for scalar primitives. Each impl is concrete, so it
/// stays disjoint from the per-model impls the derive macro generates.
macro_rules! impl_via_many_scalar {
    ( $( $t:ty ),* $(,)? ) => {
        $(
            impl ViaTarget for $t {
                type Query = Query<List<$t>>;
                type Path<Origin> = Path<Origin, List<$t>>;

                fn new_path<Origin>(path: Path<Origin, List<$t>>) -> Self::Path<Origin> {
                    // A scalar terminal is a leaf — the path is the handle.
                    path
                }

                fn via_field_ty(singular: Name, path: stmt::Path) -> FieldTy {
                    // The terminal scalar field is the path's last step.
                    let terminal = *path
                        .projection
                        .as_slice()
                        .last()
                        .expect("via path has at least one step");
                    let expr_ty = stmt::Type::List(Box::new(<$t as Load>::ty()));
                    // `target` (the model the relation chain reaches) is resolved
                    // in core's link phase; leave a placeholder it overwrites.
                    FieldTy::Via(Via::new(
                        UNRESOLVED_TARGET,
                        expr_ty,
                        app::Cardinality::Many { singular },
                        path,
                        Some(terminal),
                    ))
                }

                fn make_via_query(assoc: Association<List<$t>>) -> Self::Query {
                    // A via-as-source query whose source model and terminal
                    // projection RewriteVia fills in from the resolved schema
                    // (see its scalar branch). The placeholder source id is
                    // overwritten there. `distinct` collapses duplicate terminal
                    // values, matching the include path.
                    let mut query = stmt::Query::builder(stmt::SourceModel {
                        id: UNRESOLVED_TARGET,
                        via: Some(assoc.untyped),
                    })
                    .build();
                    query.body.as_select_mut_unwrap().distinct = true;
                    Query::from_untyped(query)
                }
            }
        )*
    };
}

impl_via_many_scalar!(
    String,
    bool,
    i8,
    i16,
    i32,
    i64,
    u16,
    u32,
    u64,
    isize,
    usize,
    f32,
    f64,
    uuid::Uuid,
);

#[cfg(feature = "rust_decimal")]
impl_via_many_scalar!(rust_decimal::Decimal);

#[cfg(feature = "bigdecimal")]
impl_via_many_scalar!(bigdecimal::BigDecimal);
