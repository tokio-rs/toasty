mod create;
mod docs;
mod embedded_enum;
mod fields;
mod filters;
mod model;
mod query;
mod relation;
mod schema;
mod update;
mod upsert;
mod util;

use filters::Filter;
use upsert::Upsert;

use super::schema::{FieldTy, Model, ModelKind};

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

struct Expand<'a> {
    /// The model being expanded
    model: &'a Model,

    /// Model filter methods
    filters: Vec<Filter>,

    /// Model upsert builders
    upserts: Vec<Upsert>,

    /// Path prefix for toasty types
    toasty: TokenStream,
}

impl Expand<'_> {
    fn expand(&self) -> TokenStream {
        let model_impls = self.expand_model_impls();
        let model_field_struct = self.expand_field_struct();
        let model_field_list_struct = self.expand_field_list_struct();
        let query_struct = self.expand_query_struct();
        let create_builder = self.expand_create_builder();
        let update_builder = self.expand_update_builder();
        let upsert_builders = self.expand_upsert_builders();
        let storage_compat_checks = self.expand_storage_compat_checks();
        let column_type_requirement_checks = self.expand_column_type_requirement_checks();
        let auto_compat_checks = self.expand_auto_compat_checks();
        let version_compat_checks = self.expand_version_compat_checks();
        let indexable_checks = self.expand_indexable_checks();

        wrap_in_const(quote! {
            #model_impls
            #model_field_struct
            #model_field_list_struct
            #query_struct
            #create_builder
            #update_builder
            #upsert_builders
            #storage_compat_checks
            #column_type_requirement_checks
            #auto_compat_checks
            #version_compat_checks
            #indexable_checks
        })
    }
}

pub(super) fn root_model(model: &Model) -> TokenStream {
    let toasty = quote!(_toasty::codegen_support);

    Expand {
        model,
        filters: Filter::build_model_filters(model),
        upserts: Upsert::build_model_upserts(model),
        toasty,
    }
    .expand()
}

pub(super) fn embedded_model(model: &Model) -> TokenStream {
    let toasty = quote!(_toasty::codegen_support);
    let model_ident = &model.ident;
    let embedded = model.kind.as_embedded_unwrap();
    let field_struct_ident = &embedded.field_struct_ident;
    let update_struct_ident = &embedded.update_struct_ident;
    let fields_named = embedded.fields_named;

    let expand = Expand {
        model,
        filters: vec![],
        upserts: vec![],
        toasty: toasty.clone(),
    };

    let model_schema = expand.expand_model_schema();
    let field_register_calls = expand.expand_field_register_calls();
    let into_expr_body_val = expand.expand_embedded_into_expr_body(fields_named, false);
    let into_expr_body_ref = expand.expand_embedded_into_expr_body(fields_named, true);
    let load_body = expand.expand_load_body(fields_named);
    let reload_body = expand.expand_embedded_reload_body(embedded.fields_named);
    let embedded_field_struct = expand.expand_field_struct();
    let embedded_field_list_struct = expand.expand_field_list_struct();
    let embedded_model_impls = expand.expand_embedded_model_impls();
    let embedded_update_builder = expand.expand_embedded_update_builder();
    let storage_compat_checks = expand.expand_storage_compat_checks();
    let column_type_requirement_checks = expand.expand_column_type_requirement_checks();
    let indexable_checks = expand.expand_indexable_checks();
    let newtype_marker = expand.expand_embedded_newtype_marker();
    let newtype_indexable_impl = expand.expand_embedded_indexable_impl();
    let field_list_struct_ident = &embedded.field_list_struct_ident;

    wrap_in_const(quote! {
        #newtype_marker
        #newtype_indexable_impl

        #embedded_field_struct
        #embedded_field_list_struct

        #embedded_update_builder

        #embedded_model_impls

        #storage_compat_checks
        #column_type_requirement_checks
        #indexable_checks

        impl #toasty::Embed for #model_ident {
            fn id() -> #toasty::core::schema::app::ModelId {
                static ID: std::sync::OnceLock<#toasty::core::schema::app::ModelId> = std::sync::OnceLock::new();
                *ID.get_or_init(|| #toasty::generate_unique_id())
            }

            #model_schema
        }

        impl #toasty::Load for #model_ident {
            type Output = Self;

            fn ty() -> #toasty::core::stmt::Type {
                #toasty::core::stmt::Type::Model(<Self as #toasty::Embed>::id())
            }

            fn load(value: #toasty::core::stmt::Value) -> #toasty::Result<Self> {
                #load_body
            }

            fn reload(target: &mut Self, value: #toasty::core::stmt::Value) -> #toasty::Result<()> {
                #reload_body
            }
        }

        impl #toasty::Field for #model_ident {
            type ExprTarget = Self;
            type Path<__Origin> = #field_struct_ident<__Origin>;
            type ListPath<__Origin> = #field_list_struct_ident<__Origin>;
            type Update<'a> = #update_struct_ident<'a>;
            type Inner = Self;

            fn new_path<__Origin>(path: #toasty::Path<__Origin, Self>) -> Self::Path<__Origin> {
                #field_struct_ident { path }
            }

            fn new_list_path<__Origin>(path: #toasty::Path<__Origin, #toasty::List<Self::ExprTarget>>) -> Self::ListPath<__Origin> {
                #field_list_struct_ident { path }
            }

            fn new_update<'a>(
                assignments: &'a mut #toasty::core::stmt::Assignments,
                projection: #toasty::core::stmt::Projection,
            ) -> Self::Update<'a> {
                #update_struct_ident { assignments, projection }
            }

            fn field_ty(
                storage_ty: Option<#toasty::core::schema::db::Type>,
            ) -> #toasty::core::schema::app::FieldTy {
                #toasty::core::schema::app::FieldTy::Embedded(
                    #toasty::core::schema::app::Embedded {
                        target: <Self as #toasty::Embed>::id(),
                        expr_ty: <Self as #toasty::Load>::ty(),
                        storage_ty,
                    }
                )
            }

            fn key_constraint<__Origin>(
                &self,
                target: #toasty::Path<__Origin, Self::Inner>,
            ) -> #toasty::stmt::Expr<bool> {
                target.eq(self)
            }

            fn register(model_set: &mut #toasty::core::schema::app::ModelSet) {
                if model_set.contains(<Self as #toasty::Embed>::id()) {
                    return;
                }
                model_set.add(<Self as #toasty::Embed>::schema());
                #( #field_register_calls )*
            }
        }

        // A struct embed can be stored as a `#[document]` column (an enum
        // embed cannot, yet — its document encoding is undefined). The
        // `#[document]` attribute resolves the field's type through this
        // trait, so the bound is what rejects the attribute on
        // non-document-capable types at compile time.
        impl #toasty::Document for #model_ident {}

        impl #toasty::stmt::IntoExpr<#model_ident> for #model_ident {
            fn into_expr(self) -> #toasty::stmt::Expr<#model_ident> {
                #into_expr_body_val
            }

            fn by_ref(&self) -> #toasty::stmt::Expr<#model_ident> {
                #into_expr_body_ref
            }
        }

        impl #toasty::Assign<#model_ident> for #model_ident {
            fn into_assignment(self) -> #toasty::stmt::Assignment<#model_ident> {
                #toasty::stmt::set(
                    <Self as #toasty::IntoExpr<#model_ident>>::into_expr(self)
                )
            }
        }
    })
}

pub(super) fn embedded_enum(model: &Model) -> TokenStream {
    let toasty = quote!(_toasty::codegen_support);
    let model_ident = &model.ident;

    let e = Expand {
        model,
        filters: vec![],
        upserts: vec![],
        toasty: toasty.clone(),
    };

    let name = schema::expand_name(&toasty, &model.name);
    let variant_tokens = e.expand_enum_variants();
    let field_tokens = e.expand_enum_schema_fields();
    let indices = e.expand_model_indices();
    let into_expr_arms = e.expand_enum_into_expr_arms();
    let load_impl = e.expand_enum_load_impl();

    let embedded_enum = model.kind.as_embedded_enum_unwrap();
    let disc_ty = if embedded_enum.uses_string_discriminants() {
        quote! { #toasty::core::stmt::Type::String }
    } else {
        quote! { #toasty::core::stmt::Type::I64 }
    };

    // Generate the storage_ty token for the discriminant FieldPrimitive.
    let storage_ty_tokens = e.expand_enum_storage_ty();

    let field_struct_ident = &embedded_enum.field_struct_ident;
    let field_list_struct_ident = &embedded_enum.field_list_struct_ident;
    let enum_field_struct = e.expand_enum_field_struct();
    let enum_field_list_struct = e.expand_field_list_struct();
    let field_register_calls = e.expand_field_register_calls();
    let storage_compat_checks = e.expand_storage_compat_checks();
    let column_type_requirement_checks = e.expand_column_type_requirement_checks();
    let discriminant_storage_compat_impls = e.expand_enum_discriminant_compat_impls();
    let shared_column_checks = e.expand_shared_column_checks();
    let indexable_checks = e.expand_indexable_checks();

    // A unit (data-less) enum is a single scalar discriminant: indexable, and a
    // valid `Vec<Enum>` element (`Scalar` unlocks the container operators).
    // Data-carrying enums span multiple columns and get neither.
    let unit_enum_impls = if model.fields.is_empty() {
        quote! {
            impl #toasty::index::IndexableField for #model_ident {}
            impl #toasty::Scalar for #model_ident {}
        }
    } else {
        quote! {}
    };

    wrap_in_const(quote! {
        #enum_field_struct
        #enum_field_list_struct

        #storage_compat_checks
        #column_type_requirement_checks
        #discriminant_storage_compat_impls
        #shared_column_checks
        #indexable_checks
        #unit_enum_impls

        impl #toasty::Embed for #model_ident {
            fn id() -> #toasty::core::schema::app::ModelId {
                static ID: std::sync::OnceLock<#toasty::core::schema::app::ModelId> = std::sync::OnceLock::new();
                *ID.get_or_init(|| #toasty::generate_unique_id())
            }

            fn schema() -> #toasty::core::schema::app::Model {
                let id = <Self as #toasty::Embed>::id();
                #toasty::core::schema::app::Model::EmbeddedEnum(
                    #toasty::core::schema::app::EmbeddedEnum {
                        id,
                        name: #name,
                        discriminant: #toasty::core::schema::app::FieldPrimitive {
                            ty: #disc_ty,
                            storage_ty: #storage_ty_tokens,
                            serialize: ::std::option::Option::None,
                        },
                        variants: vec![ #( #variant_tokens ),* ],
                        fields: vec![ #( #field_tokens ),* ],
                        indices: #indices,
                    }
                )
            }
        }

        #load_impl

        impl #toasty::Field for #model_ident {
            type ExprTarget = Self;
            type Path<__Origin> = #field_struct_ident<__Origin>;
            type ListPath<__Origin> = #field_list_struct_ident<__Origin>;
            type Update<'a> = ();
            type Inner = Self;

            fn new_path<__Origin>(path: #toasty::Path<__Origin, Self>) -> Self::Path<__Origin> {
                #field_struct_ident { path }
            }

            fn new_list_path<__Origin>(path: #toasty::Path<__Origin, #toasty::List<Self::ExprTarget>>) -> Self::ListPath<__Origin> {
                #field_list_struct_ident { path }
            }

            fn new_update<'a>(
                _assignments: &'a mut #toasty::core::stmt::Assignments,
                _projection: #toasty::core::stmt::Projection,
            ) -> Self::Update<'a> {
            }

            fn field_ty(
                storage_ty: Option<#toasty::core::schema::db::Type>,
            ) -> #toasty::core::schema::app::FieldTy {
                #toasty::core::schema::app::FieldTy::Embedded(
                    #toasty::core::schema::app::Embedded {
                        target: <Self as #toasty::Embed>::id(),
                        expr_ty: <Self as #toasty::Load>::ty(),
                        storage_ty,
                    }
                )
            }

            fn key_constraint<__Origin>(
                &self,
                target: #toasty::Path<__Origin, Self::Inner>,
            ) -> #toasty::stmt::Expr<bool> {
                target.eq(self)
            }

            fn register(model_set: &mut #toasty::core::schema::app::ModelSet) {
                if model_set.contains(<Self as #toasty::Embed>::id()) {
                    return;
                }
                model_set.add(<Self as #toasty::Embed>::schema());
                #( #field_register_calls )*
            }
        }

        impl #toasty::stmt::IntoExpr<#model_ident> for #model_ident {
            fn into_expr(self) -> #toasty::stmt::Expr<#model_ident> {
                match self { #( #into_expr_arms )* }
            }

            fn by_ref(&self) -> #toasty::stmt::Expr<#model_ident> {
                match self { #( #into_expr_arms )* }
            }
        }

        impl #toasty::Assign<#model_ident> for #model_ident {
            fn into_assignment(self) -> #toasty::stmt::Assignment<#model_ident> {
                #toasty::stmt::set(
                    <Self as #toasty::IntoExpr<#model_ident>>::into_expr(self)
                )
            }
        }
    })
}

// === Shared token-generation helpers ===

impl Expand<'_> {
    /// For tuple-newtype `#[derive(Embed)]` types (one unnamed field), emit
    /// the `NewtypeOf` marker carrying the inner field's type. The blanket
    /// `impl<T: NewtypeOf, T::Inner: Auto> Auto for T` in `codegen_support`
    /// then promotes the newtype to `Auto` whenever the inner type is auto,
    /// without errors when the inner type is not auto.
    fn expand_embedded_newtype_marker(&self) -> TokenStream {
        let ModelKind::EmbeddedStruct(embedded) = &self.model.kind else {
            return quote! {};
        };
        // Only canonical newtypes (single unnamed field) qualify. Named
        // single-field structs are explicit wrappers and stay opaque.
        if embedded.fields_named || self.model.fields.len() != 1 {
            return quote! {};
        }

        let inner = &self.model.fields[0];
        let FieldTy::Primitive(inner_ty) = &inner.ty else {
            // Relations are not allowed inside an `Embed` body today; nothing
            // to mark if that ever changes.
            return quote! {};
        };

        let toasty = &self.toasty;
        let model_ident = &self.model.ident;

        quote! {
            impl #toasty::newtype::NewtypeOf for #model_ident {
                type Inner = #inner_ty;

                fn into_inner(self) -> #inner_ty {
                    self.0
                }

                fn from_inner(inner: #inner_ty) -> Self {
                    Self(inner)
                }
            }
        }
    }

    /// For tuple-newtype `#[derive(Embed)]` structs, emit an `IndexableField`
    /// impl that forwards to the inner type, so a newtype wrapping an indexable
    /// scalar can itself serve as an index column. Multi-field and named
    /// single-field structs are opaque wrappers and stay non-indexable.
    ///
    /// This is a per-type impl rather than a `NewtypeOf` blanket: a blanket
    /// would conflict with the `Box<T>` forwarding impl in
    /// `codegen_support::index`, because `Box` is `#[fundamental]`.
    fn expand_embedded_indexable_impl(&self) -> TokenStream {
        let ModelKind::EmbeddedStruct(embedded) = &self.model.kind else {
            return quote! {};
        };
        if embedded.fields_named || self.model.fields.len() != 1 {
            return quote! {};
        }

        let FieldTy::Primitive(inner_ty) = &self.model.fields[0].ty else {
            return quote! {};
        };

        let toasty = &self.toasty;
        let model_ident = &self.model.ident;

        quote! {
            impl #toasty::index::IndexableField for #model_ident
            where
                #inner_ty: #toasty::index::IndexableField,
            {}
        }
    }

    /// Generates a field accessor method for a `BelongsTo` or `HasOne`
    /// relation using the target model's `Model::OneField`.
    fn expand_one_relation_field_method(
        &self,
        field_ident: &syn::Ident,
        field_trait: TokenStream,
        ty: &syn::Type,
        field_offset: &TokenStream,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let schema_trait = self.schema_trait();
        let span = field_ident.span();

        quote_spanned! { span=>
            #vis fn #field_ident(&self) -> <<#ty as #field_trait>::Target as #toasty::Model>::OneField<__Origin> {
                <<<#ty as #field_trait>::Target as #toasty::Model>::OneField<__Origin>>::from_path(
                    self.path().chain(
                        <#model_ident as #schema_trait>::path_field(#field_offset)
                    )
                )
            }
        }
    }

    /// Generates a field accessor method for a primitive field, resolving the
    /// path shape through the field type's [`Field`] impl — its `Path` /
    /// `new_path` / `ExprTarget`. The type itself decides its path shape (a
    /// struct embed's Fields handle, a `Vec<scalar>` / `Vec<Embed>` list leaf)
    /// without the macro inspecting the Rust type. A `#[document]` field uses
    /// the same `Field` impl as its column-expanded form.
    fn expand_primitive_field_method(
        &self,
        field_ident: &syn::Ident,
        ty: &syn::Type,
        field_offset: &TokenStream,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let schema_trait = self.schema_trait();
        let span = field_ident.span();

        // Construct the chained path with the field's `ExprTarget` as the
        // tag, so `new_path` receives exactly the type it expects. For
        // `Vec<_>` this is `List<T>`; for everything else it is the field's
        // Rust type.
        quote_spanned! { span=>
            #vis fn #field_ident(&self) -> <#ty as #toasty::Field>::Path<__Origin> {
                <#ty as #toasty::Field>::new_path(
                    self.path().chain(
                        <#model_ident as #schema_trait>::path_field::<<#ty as #toasty::Field>::ExprTarget>(#field_offset)
                    )
                )
            }
        }
    }
}

fn wrap_in_const(code: TokenStream) -> TokenStream {
    quote! {
        const _: () = {
            use toasty as _toasty;
            // Import the setter-bound names unqualified so the `impl Trait`
            // parameter types on create/update setters render as
            // `impl IntoExpr<FieldExprTarget<..>>` in compiler errors rather
            // than the much longer `_toasty::codegen_support::..` paths. Not
            // every model uses all three (a model with only relation setters
            // never names `Assign` here), so silence the unused-import lint.
            #[allow(unused_imports)]
            use _toasty::codegen_support::{Assign, FieldExprTarget, IntoExpr};
            #code
        };
    }
}
