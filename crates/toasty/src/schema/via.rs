use super::{Deferred, Load};
use crate::stmt::{Association, List, Query};

use toasty_core::schema::Name;
use toasty_core::schema::app::{self, FieldTy, ModelId, Via};
use toasty_core::stmt;

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

    /// Build the [`FieldTy::Via`] for a `#[has_many(via = …)]` field whose
    /// terminal element type is `Self`.
    ///
    /// `path` is the fully-resolved field path (rooted at the declaring
    /// model), including the terminal step. `terminal_owner` is the model that
    /// owns the path's last segment: a scalar terminal uses it as the via
    /// target, while a model terminal ignores it in favour of `Self::id()`.
    fn via_field_ty(singular: Name, path: stmt::Path, terminal_owner: ModelId) -> FieldTy;

    /// Wrap a via-field association into the navigation query.
    ///
    /// `target` and `terminal` describe a scalar terminal (the model the
    /// relation chain reaches and the projected field's index on it); a model
    /// terminal ignores them.
    fn make_via_query(
        assoc: Association<List<Self>>,
        target: ModelId,
        terminal: usize,
    ) -> Self::Query
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

                fn via_field_ty(
                    singular: Name,
                    path: stmt::Path,
                    terminal_owner: ModelId,
                ) -> FieldTy {
                    // The terminal scalar field is the path's last step, on the
                    // model the relation chain reaches.
                    let terminal = *path
                        .projection
                        .as_slice()
                        .last()
                        .expect("via path has at least one step");
                    let expr_ty = stmt::Type::List(Box::new(<$t as Load>::ty()));
                    FieldTy::Via(Via::new(
                        terminal_owner,
                        expr_ty,
                        app::Cardinality::Many { singular },
                        path,
                        Some(terminal),
                    ))
                }

                fn make_via_query(
                    assoc: Association<List<$t>>,
                    target: ModelId,
                    terminal: usize,
                ) -> Self::Query {
                    // Iterate the via target's rows (the via association is
                    // unfolded into a reachability filter during lowering) and
                    // project the terminal field — the same shape as
                    // `.select(Target::fields().field())`. Built like a
                    // relation-via source query (`Query::builder(SourceModel)`)
                    // so it lowers identically, then the returning is set to the
                    // terminal column. `distinct` collapses duplicate terminal
                    // values, matching the include path.
                    let mut query = stmt::Query::builder(stmt::SourceModel {
                        id: target,
                        via: Some(assoc.untyped),
                    })
                    .build();
                    let select = query.body.as_select_mut_unwrap();
                    select.returning = stmt::Returning::Project(
                        stmt::Path::field(target, terminal).into_stmt(),
                    );
                    select.distinct = true;
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
