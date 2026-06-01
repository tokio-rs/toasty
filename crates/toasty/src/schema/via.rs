use super::{Deferred, Load};
use crate::stmt::{Association, List, Query};

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

    /// The query builder the generated navigation methods return. Forwards to
    /// the terminal's [`ViaTarget::Query`] so the derive can name it in one hop
    /// (`<Field as ViaManyField>::Query`) instead of projecting through both
    /// traits.
    type Query;

    /// Whether the field stores its value in a deferred load slot.
    const DEFERRED: bool;

    /// Build the field's [`FieldTy::Via`]. Forwards to
    /// [`ViaTarget::via_field_ty`] on the terminal type.
    fn via_field_ty(singular: Name, path: stmt::Path) -> FieldTy {
        <Self::Target as ViaTarget>::via_field_ty(singular, path)
    }

    /// Wrap a via-field association into the navigation query. Forwards to
    /// [`ViaTarget::make_via_query`] on the terminal type.
    fn make_via_query(assoc: Association<List<Self::Target>>) -> Self::Query;
}

impl<E: ViaTarget> ViaManyField for Vec<E> {
    type Target = E;
    type Query = <E as ViaTarget>::Query;
    const DEFERRED: bool = false;

    fn make_via_query(assoc: Association<List<E>>) -> Self::Query {
        <E as ViaTarget>::make_via_query(assoc)
    }
}

impl<E: ViaTarget> ViaManyField for Deferred<Vec<E>> {
    type Target = E;
    type Query = <E as ViaTarget>::Query;
    const DEFERRED: bool = true;

    fn make_via_query(assoc: Association<List<E>>) -> Self::Query {
        <E as ViaTarget>::make_via_query(assoc)
    }
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
    /// model), including the terminal step. The via target is `Self::id()` for
    /// a model terminal; for a scalar terminal it is the model the relation
    /// chain reaches, which the path alone can't name here — so the scalar
    /// impls leave it unset and `toasty-core` fills it in while linking.
    fn via_field_ty(singular: Name, path: stmt::Path) -> FieldTy;

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
