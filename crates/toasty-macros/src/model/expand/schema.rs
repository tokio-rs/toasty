use super::{Expand, util};
use crate::model::schema::{AutoStrategy, Column, FieldTy, ModelKind, Name, UuidVersion};

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

impl Expand<'_> {
    pub(super) fn expand_model_schema(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        let name = expand_name(toasty, &self.model.name);
        let fields = self.expand_model_fields();
        let indices = self.expand_model_indices();
        let table_name = self.expand_table_name();

        let model = match &self.model.kind {
            ModelKind::Root(root) => {
                let primary_key = self.expand_primary_key();
                let version_field = match root.version_field {
                    Some(idx) => {
                        let idx_tok = util::int(idx);
                        quote! {
                            Some(#toasty::core::schema::app::FieldId {
                                model: id,
                                index: #idx_tok,
                            })
                        }
                    }
                    None => quote! { None },
                };
                quote! {
                    #toasty::core::schema::app::Model::Root(
                        #toasty::core::schema::app::ModelRoot {
                            id,
                            name: #name,
                            fields: #fields,
                            primary_key: #primary_key,
                            table_name: #table_name,
                            indices: #indices,
                            version_field: #version_field,
                        }
                    )
                }
            }
            ModelKind::EmbeddedStruct(_) => {
                quote! {
                    #toasty::core::schema::app::Model::EmbeddedStruct(
                        #toasty::core::schema::app::EmbeddedStruct {
                            id,
                            name: #name,
                            fields: #fields,
                            indices: #indices,
                        }
                    )
                }
            }
            ModelKind::EmbeddedEnum(_) => {
                panic!("expand_model_schema called on EmbeddedEnum; use embedded_enum() instead")
            }
        };

        // `id()` lives on `Model` for root models and on `Embed` for embedded
        // structs; qualify the call so it resolves regardless of which trait
        // impl this `schema()` body is emitted into.
        let id_trait = match &self.model.kind {
            ModelKind::Root(_) => quote!(#toasty::Model),
            ModelKind::EmbeddedStruct(_) => quote!(#toasty::Embed),
            ModelKind::EmbeddedEnum(_) => unreachable!(),
        };

        quote! {
            fn schema() -> #toasty::core::schema::app::Model {
                let id = <#model_ident as #id_trait>::id();

                #model
            }
        }
    }

    fn expand_model_fields(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;

        let fields = self.model.fields.iter().enumerate().map(|(index, field)| {
            let index_tokenized = util::int(index);
            let field_ty;
            let nullable;
            let deferred;

            let field_named = match &self.model.kind {
                ModelKind::Root(_) => true,
                ModelKind::EmbeddedStruct(model_embedded_struct) => model_embedded_struct.fields_named,
                ModelKind::EmbeddedEnum(model_embedded_enum) => {
                    let variant = field.variant.unwrap();
                    model_embedded_enum.variants[variant].fields_named
                }
            };

            let name = {
                let app_name = if field_named {
                    let n = field.name.as_str();
                    quote! { Some(#n.to_string()) }
                } else {
                    quote! { None }
                };

                let storage_name = match field.attrs.column.as_ref().and_then(|column| column.name.as_ref()) {
                    Some(name) => quote! { Some(#name.to_string()) },
                    None => quote! { None },
                };

                quote! {
                    #toasty::core::schema::app::FieldName {
                        app: #app_name,
                        storage: #storage_name,
                    }
                }
            };

            match &field.ty {
                FieldTy::Primitive(ty) => {
                    let storage_ty = match &field.attrs.column {
                        Some(Column { ty: Some(col_ty), ..}) => {
                            let expanded = col_ty.expand_with(toasty);
                            quote!(Some(#expanded))
                        }
                        _ => quote!(None),
                    };

                    nullable = quote!(<#ty as #toasty::Field>::NULLABLE);
                    deferred = quote!(<#ty as #toasty::Field>::DEFERRED);

                    // A `#[document]` field is stored as a single JSON column.
                    // Its app type resolves through the `Document` trait —
                    // `Model(id)` for a struct embed, `List(Model(id))` for a
                    // `Vec<embed>` collection — which the schema builder later
                    // resolves to a `Document` once every embed is registered.
                    // The trait bound also rejects `#[document]` on any type
                    // that cannot use document storage (a scalar, an enum
                    // embed, a `Vec<scalar>`) at compile time. A non-document
                    // field uses `Field::field_ty`, which column-expands a
                    // struct embed (`Embedded`) and leaves scalars /
                    // `Vec<scalar>` as a `Primitive`.
                    field_ty = if field.attrs.document.is_some() {
                        quote! {
                            #toasty::core::schema::app::FieldTy::Primitive(
                                #toasty::core::schema::app::FieldPrimitive {
                                    ty: <#ty as #toasty::Document>::document_ty(),
                                    storage_ty: #storage_ty,
                                    serialize: None,
                                }
                            )
                        }
                    } else {
                        quote!(<#ty as #toasty::Field>::field_ty(#storage_ty))
                    };
                }
                FieldTy::BelongsTo(rel) => {
                    let ty = &rel.ty;

                    let fk_fields = rel.foreign_key.iter().map(|fk_field| {
                        let source = util::int(fk_field.source);
                        let target = fk_field.target.to_string();

                        quote! {
                            #toasty::core::schema::app::ForeignKeyField {
                                source: #toasty::core::schema::app::FieldId {
                                    model: #model_ident::id(),
                                    index: #source,
                                },
                                target: {
                                    type __RelationTarget = <#ty as #toasty::RelationOneField>::Target;
                                    <__RelationTarget as #toasty::Model>::field_name_to_id(#target)
                                },
                            }
                        }
                    });

                    nullable = quote!(<#ty as #toasty::RelationOneField>::NULLABLE);
                    deferred = quote!(<#ty as #toasty::RelationOneField>::DEFERRED);
                    field_ty = quote!(<#ty as #toasty::RelationOneField>::belongs_to_relation_field_ty(
                        #toasty::core::schema::app::ForeignKey {
                            fields: vec![ #( #fk_fields ),* ],
                        },
                    ));
                }
                FieldTy::HasMany(rel) => {
                    let ty = &rel.ty;
                    let singular_name = expand_name(toasty, &rel.singular);

                    if let Some(segments) = &rel.via {
                        // A `via` field routes through `ViaTarget`, keyed on
                        // the terminal element type, so it works whether the
                        // terminal is a model or a scalar — without requiring
                        // `RelationManyField` (which needs the element to be a
                        // model).
                        let terminal_ty =
                            quote!(#toasty::List<<#ty as #toasty::ViaManyField>::Target>);
                        let full_path =
                            expand_via_path(toasty, model_ident, segments, &terminal_ty);

                        // A has-many collection is always present, never null.
                        nullable = quote!(false);
                        deferred = quote!(<#ty as #toasty::ViaManyField>::DEFERRED);
                        field_ty = quote!(
                            <<#ty as #toasty::ViaManyField>::Target as #toasty::ViaTarget>::via_field_ty(
                                #singular_name, #full_path,
                            )
                        );
                    } else {
                        let pair = expand_pair(toasty, quote!(#toasty::RelationManyField), ty, rel.pair.as_ref());

                        // A has-many collection is always present, never null.
                        nullable = quote!(false);
                        deferred = quote!(<#ty as #toasty::RelationManyField>::DEFERRED);
                        field_ty = quote!(<#ty as #toasty::RelationManyField>::many_relation_field_ty(#singular_name, #pair, None));
                    }
                }
                FieldTy::HasOne(rel) => {
                    let ty = &rel.ty;
                    let pair = expand_pair(toasty, quote!(#toasty::RelationOneField), ty, rel.pair.as_ref());
                    // A has-one via reaches a single model; pin the path's
                    // terminal to that model so a mismatched declaration is a
                    // compile error rather than a runtime load failure.
                    let via = expand_via(
                        toasty,
                        model_ident,
                        rel.via.as_ref(),
                        &quote!(<#ty as #toasty::RelationOneField>::Target),
                    );

                    nullable = quote!(<#ty as #toasty::RelationOneField>::NULLABLE);
                    deferred = quote!(<#ty as #toasty::RelationOneField>::DEFERRED);
                    field_ty = quote!(<#ty as #toasty::RelationOneField>::has_one_relation_field_ty(#pair, #via));
                }
            }

            let primary_key = self.model.primary_key_fields()
                .map(|mut fields| fields.any(|f| self.model.fields.iter().position(|field| std::ptr::eq(field, f)) == Some(index)))
                .unwrap_or(false);
            let auto = match &field.attrs.auto {
                None => quote! { None },
                Some(auto) => {
                    let FieldTy::Primitive(ty) = &field.ty else { todo!("better error handling") };

                    match auto {
                        AutoStrategy::Unspecified => {
                            assert!(primary_key, "TODO: better error handling");

                            quote! { Some(<#ty as #toasty::Auto>::STRATEGY) }
                         }
                        AutoStrategy::Uuid(UuidVersion::V4) => quote! { Some(#toasty::core::schema::app::AutoStrategy::Uuid(#toasty::core::schema::app::UuidVersion::V4)) },
                        AutoStrategy::Uuid(UuidVersion::V7) => quote! { Some(#toasty::core::schema::app::AutoStrategy::Uuid(#toasty::core::schema::app::UuidVersion::V7)) },
                        AutoStrategy::Increment => quote! { Some(#toasty::core::schema::app::AutoStrategy::Increment) },
                    }
                }
            };

            let versionable = field.attrs.versionable;

            quote! {
                #toasty::core::schema::app::Field {
                    id: #toasty::core::schema::app::FieldId {
                        model: #model_ident::id(),
                        index: #index_tokenized,
                    },
                    name: #name,
                    ty: #field_ty,
                    nullable: #nullable,
                    primary_key: #primary_key,
                    auto: #auto,
                    versionable: #versionable,
                    deferred: #deferred,
                    constraints: vec![],
                    variant: None,
                    shared: None,
                }
            }
        });

        quote! {
            vec![ #( #fields ),* ]
        }
    }

    fn expand_primary_key(&self) -> TokenStream {
        let toasty = &self.toasty;
        let primary_key = match &self.model.kind {
            ModelKind::Root(root) => &root.primary_key,
            ModelKind::EmbeddedStruct(_) => panic!("expand_primary_key called on embedded struct"),
            ModelKind::EmbeddedEnum(_) => panic!("expand_primary_key called on embedded enum"),
        };

        let fields = primary_key
            .fields
            .iter()
            .map(|field| {
                let field_tokenized = util::int(*field);

                quote! {
                    #toasty::core::schema::app::FieldId {
                        model: id,
                        index: #field_tokenized,
                    }
                }
            })
            .collect::<Vec<_>>();

        quote! {
            #toasty::core::schema::app::PrimaryKey {
                fields: vec![ #( #fields ),* ],
                index: #toasty::core::schema::app::IndexId {
                    model: id,
                    index: 0,
                },
            }
        }
    }

    pub(super) fn expand_model_indices(&self) -> TokenStream {
        use crate::model::schema::IndexScope;

        let toasty = &self.toasty;

        let indices = self
            .model
            .indices
            .iter()
            .enumerate()
            .map(|(index, model_index)| {
                let index_tokenized = util::int(index);
                let unique = &model_index.unique;
                let primary_key = &model_index.primary_key;
                let name = match &model_index.name {
                    Some(value) => quote!(Some(#value.to_string())),
                    None => quote!(None),
                };

                let fields = model_index.fields.iter().map(|index_field| {
                    let field_tokenized = util::int(index_field.field);
                    let scope = match &index_field.scope {
                        IndexScope::Partition => {
                            quote!(#toasty::core::schema::db::IndexScope::Partition)
                        }
                        IndexScope::Local => quote!(#toasty::core::schema::db::IndexScope::Local),
                    };

                    quote! {
                        #toasty::core::schema::app::IndexField {
                            field: #toasty::core::schema::app::FieldId {
                                model: id,
                                index: #field_tokenized,
                            },
                            op: #toasty::core::schema::db::IndexOp::Eq,
                            scope: #scope,
                        }
                    }
                });

                quote! {
                    #toasty::core::schema::app::Index {
                        id: #toasty::core::schema::app::IndexId {
                            model: id,
                            index: #index_tokenized,
                        },
                        name: #name,
                        fields: vec![ #( #fields ),* ],
                        unique: #unique,
                        primary_key: #primary_key,
                    }
                }
            });

        quote! {
            vec![ #( #indices ),* ]
        }
    }

    fn expand_table_name(&self) -> TokenStream {
        let table_name = match &self.model.table {
            Some(table_name) => table_name.value(),
            // Derive the default table name at compile time so building the
            // schema at runtime never pays the cost of the `pluralizer` crate's
            // lazy regex compilation: snake_case the model name, then pluralize.
            // The table-name prefix, if any, is applied at runtime by the builder.
            None => pluralizer::pluralize(&self.model.name.snake_case, 2, false),
        };

        quote! { #table_name.to_string() }
    }
}

impl Expand<'_> {
    /// Emit one obligation per primitive field with `#[column(type = ...)]`
    /// that the field's Rust type implements
    /// `codegen_support::storage::CompatibleWith<Tag>` for the matching tag.
    ///
    /// The check leans on the Rust type checker — the macro does not inspect
    /// the field type, it only names it. Type aliases, re-exports, and
    /// generic parameters resolve through normal trait resolution.
    pub(super) fn expand_storage_compat_checks(&self) -> TokenStream {
        let toasty = &self.toasty;

        let checks = self.model.fields.iter().filter_map(|field| {
            let FieldTy::Primitive(ty) = &field.ty else {
                return None;
            };

            let col_ty = field.attrs.column.as_ref().and_then(|c| c.ty.as_ref())?;
            let marker = col_ty.compat_marker(toasty)?;

            Some(compat_check(
                ty,
                quote! { #toasty::storage::CompatibleWith<#marker> },
            ))
        });

        quote! { #( #checks )* }
    }

    /// Reject field types that require an explicit `#[column(type = ...)]`
    /// when the attribute is absent.
    ///
    /// The field type communicates the requirement through `Field` trait
    /// dispatch. The macro only checks whether the attribute supplied a type;
    /// it never inspects the Rust type syntax. This also makes aliases and
    /// transparent wrappers behave the same as the underlying field type.
    pub(super) fn expand_column_type_requirement_checks(&self) -> TokenStream {
        let toasty = &self.toasty;

        let checks = self.model.fields.iter().filter_map(|field| {
            let FieldTy::Primitive(ty) = &field.ty else {
                return None;
            };

            if field
                .attrs
                .column
                .as_ref()
                .and_then(|column| column.ty.as_ref())
                .is_some()
            {
                return None;
            }

            Some(quote_spanned! { ty.span()=>
                const _: () = {
                    if <#ty as #toasty::Field>::REQUIRES_EXPLICIT_COLUMN_TYPE {
                        panic!(
                            "`toasty::Json<T>` fields require `#[column(type = ...)]`; use \
                             `#[column(type = text)]` for text-backed JSON storage"
                        );
                    }
                };
            })
        });

        quote! { #( #checks )* }
    }

    /// Emit one obligation per field with an explicit `#[auto(...)]` strategy
    /// that the field's Rust type implements
    /// `codegen_support::auto::AutoCompatible<Tag>` for the matching tag.
    ///
    /// The bare `#[auto]` form already gets a `T: Auto` obligation from the
    /// `STRATEGY` const lookup in `expand_model_fields`, so it does not need
    /// a separate check here.
    pub(super) fn expand_auto_compat_checks(&self) -> TokenStream {
        let toasty = &self.toasty;

        let checks = self.model.fields.iter().filter_map(|field| {
            let auto = field.attrs.auto.as_ref()?;

            let FieldTy::Primitive(ty) = &field.ty else {
                return None;
            };

            let tag = match auto {
                AutoStrategy::Unspecified => return None,
                AutoStrategy::Uuid(_) => quote! { #toasty::auto::tag::Uuid },
                AutoStrategy::Increment => quote! { #toasty::auto::tag::Increment },
            };

            Some(compat_check(
                ty,
                quote! { #toasty::auto::AutoCompatible<#tag> },
            ))
        });

        quote! { #( #checks )* }
    }

    /// Emit a compile-time obligation that every `#[version]` field's Rust type
    /// implements [`Version`].
    ///
    /// The bare `u64` type satisfies the bound directly; tuple-newtype embeds
    /// satisfy it via the blanket in `codegen_support::version`. Any other type
    /// produces a compiler error with the `#[diagnostic::on_unimplemented]`
    /// message on [`Version`].
    pub(super) fn expand_version_compat_checks(&self) -> TokenStream {
        let toasty = &self.toasty;

        let checks = self.model.fields.iter().filter_map(|field| {
            if !field.attrs.versionable {
                return None;
            }

            let FieldTy::Primitive(ty) = &field.ty else {
                return None;
            };

            Some(compat_check(ty, quote! { #toasty::Version }))
        });

        quote! { #( #checks )* }
    }

    /// Emit one obligation per field that participates in a secondary index or
    /// unique constraint (`#[index]`, `#[unique]`, or a model-level
    /// `#[index(...)]` / `#[unique(...)]`) that the field's Rust type implements
    /// `codegen_support::index::IndexableField`.
    ///
    /// Scalars satisfy the bound directly; newtype embeds via the `NewtypeOf`
    /// blanket; unit (data-less) enums via the impl emitted by
    /// `#[derive(Embed)]`. Data-carrying enums and multi-field embedded structs
    /// span multiple columns and do not implement it, so naming one in an index
    /// is a compile error instead of a runtime panic.
    ///
    /// The primary key is excluded: keys are validated through their own paths.
    pub(super) fn expand_indexable_checks(&self) -> TokenStream {
        let toasty = &self.toasty;

        let mut seen = std::collections::BTreeSet::new();
        let checks = self
            .model
            .indices
            .iter()
            .filter(|index| !index.primary_key)
            .flat_map(|index| index.fields.iter())
            .filter_map(|index_field| {
                if !seen.insert(index_field.field) {
                    return None;
                }

                let FieldTy::Primitive(ty) = &self.model.fields[index_field.field].ty else {
                    return None;
                };

                // Pin the diagnostic at the field type's span so the error
                // lands on the user's declaration, not the derive call site.
                Some(quote_spanned! { ty.span()=>
                    const _: () = {
                        fn _check<__T>()
                        where
                            __T: #toasty::index::IndexableField,
                        {}
                        let _ = _check::<#ty>;
                    };
                })
            })
            .collect::<Vec<_>>();

        quote! { #( #checks )* }
    }

    /// Generate calls to register all models reachable from this model's fields.
    ///
    /// For primitive fields, no call is emitted (the default `Field::register`
    /// is a no-op). For embedded fields, `<Type as Field>::register` is called.
    /// For relation fields (BelongsTo, HasMany, HasOne), `<TargetModel as
    /// Model>::register` is called directly.
    pub(super) fn expand_field_register_calls(&self) -> Vec<TokenStream> {
        let toasty = &self.toasty;

        self.model
            .fields
            .iter()
            .map(|field| match &field.ty {
                FieldTy::Primitive(ty) => {
                    // Both column-expanded embeds and `#[document]` fields
                    // register their embedded types through `Field::register`
                    // (a no-op for scalars).
                    quote! {
                        <#ty as #toasty::Field>::register(model_set);
                    }
                }
                FieldTy::BelongsTo(rel) => {
                    let ty = &rel.ty;
                    quote! {
                        <<#ty as #toasty::RelationOneField>::Target as #toasty::Model>::register(model_set);
                    }
                }
                FieldTy::HasMany(rel) if rel.via.is_some() => {
                    // A via relation reaches its terminal through existing
                    // relation fields, each of which registers the models it
                    // traverses; the terminal of a scalar via is not a model at
                    // all. So a via field registers nothing of its own.
                    TokenStream::new()
                }
                FieldTy::HasMany(rel) => {
                    let ty = &rel.ty;
                    quote! {
                        <<#ty as #toasty::RelationManyField>::Target as #toasty::Model>::register(model_set);
                    }
                }
                FieldTy::HasOne(rel) => {
                    let ty = &rel.ty;
                    quote! {
                        <<#ty as #toasty::RelationOneField>::Target as #toasty::Model>::register(model_set);
                    }
                }
            })
            .collect()
    }
}

/// Emit a span-pinned compile-time obligation that `ty` satisfies `bound`.
///
/// The check leans on the type checker rather than inspecting `ty`: the
/// zero-cost `_check::<#ty>` reference forces the bound to hold. `quote_spanned`
/// pins any resulting diagnostic at the field type's span so the error lands on
/// the user's declaration, not the derive call site. Shared by the
/// storage/auto/version `expand_*_compat_checks` expansions, which differ only
/// in the `bound` they require.
fn compat_check(ty: &syn::Type, bound: TokenStream) -> TokenStream {
    quote_spanned! { ty.span()=>
        const _: () = {
            fn _check<__T>()
            where
                __T: #bound,
            {}
            let _ = _check::<#ty>;
        };
    }
}

pub(super) fn expand_name(toasty: &TokenStream, name: &Name) -> TokenStream {
    let parts = name.parts.iter().map(|part| {
        let part = part.to_string();
        quote! { #part.to_string() }
    });

    quote! {
        #toasty::core::schema::Name {
            parts: vec![#( #parts ),*],
        }
    }
}

fn expand_pair(
    toasty: &TokenStream,
    field_trait: TokenStream,
    target_ty: &syn::Type,
    pair: Option<&syn::Ident>,
) -> TokenStream {
    match pair {
        Some(ident) => {
            let name = ident.to_string();
            quote! {
                Some({
                    type __RelationTarget = <#target_ty as #field_trait>::Target;
                    <__RelationTarget as #toasty::Model>::field_name_to_id(#name)
                })
            }
        }
        None => quote! { None },
    }
}

/// Emit the `via` argument for `many_relation_field_ty` / `has_one_relation_field_ty`: a
/// fully resolved [`stmt::Path`] built by chaining the named segments onto the
/// model's `Fields` struct (e.g. `User::fields().comments().article()`).
///
/// Resolution happens at Rust-compile time — a misspelled segment surfaces as
/// "no method named `foo` found for struct `UserFields`", not as a runtime
/// schema validation error. The two `.into()` conversions go via
/// `FieldsStruct: Into<Path<Origin, T>>` and `Path<T, U>: Into<stmt::Path>`;
/// the intermediate `Path<#model_ident, _>` ascription is what disambiguates
/// them.
fn expand_via(
    toasty: &TokenStream,
    model_ident: &syn::Ident,
    via: Option<&Vec<syn::Ident>>,
    terminal_ty: &TokenStream,
) -> TokenStream {
    let Some(segments) = via else {
        return quote! { None };
    };

    let path = expand_via_path(toasty, model_ident, segments, terminal_ty);
    quote! { Some(#path) }
}

/// Emit the fully-resolved [`stmt::Path`] for a `via` relation: chain the named
/// segments onto the model's `Fields` struct (e.g.
/// `User::fields().comments().article()`) and convert to an `stmt::Path`.
///
/// Resolution happens at Rust-compile time — a misspelled segment surfaces as
/// "no method named `foo` found", and an intermediate that is not navigable
/// fails to chain, so only the terminal segment may be a scalar field.
///
/// `terminal_ty` is the type the path's terminal must reach, derived from the
/// declared field (e.g. `List<i64>` for `Vec<i64>`). Pinning the typed path's
/// second parameter to it — rather than leaving it inferred — is what rejects a
/// field whose declared element type disagrees with the path
/// (`#[has_many(via = a.b.title)] x: Vec<i64>` where `title` is a `String`), so
/// the mismatch is a compile error here instead of a runtime load failure.
pub(super) fn expand_via_path(
    toasty: &TokenStream,
    model_ident: &syn::Ident,
    segments: &[syn::Ident],
    terminal_ty: &TokenStream,
) -> TokenStream {
    let mut chain = quote! { #model_ident::fields() };
    for segment in segments {
        chain = quote_spanned! { segment.span()=> #chain.#segment() };
    }

    // Pin the typed path's terminal to `terminal_ty`. When it disagrees with
    // the path, the `.into()` has no matching conversion. Point that failure at
    // the offending path step rather than the derive:
    //
    // - Bind the chain to a local first, so the conversion's receiver is that
    //   local (emitted at the terminal span) rather than the chain expression,
    //   whose root `Model::fields()` carries the derive call site.
    // - Span the whole target annotation — including the interpolated
    //   `terminal_ty`, which `quote_spanned!` would otherwise leave at the
    //   derive — to the terminal segment via `respan`.
    let span = segments
        .last()
        .map_or_else(proc_macro2::Span::call_site, syn::Ident::span);
    let typed_path_ty = util::respan(quote!(#toasty::Path<#model_ident, #terminal_ty>), span);
    quote_spanned! { span=>
        {
            let __via_chain = #chain;
            // The chain terminal is a `FieldList`/`ManyField` for a relation or
            // model via-of-via terminal (a real conversion to `Path`), but
            // already a `Path` for a scalar terminal, where `.into()` is
            // identity — allow that.
            #[allow(clippy::useless_conversion)]
            let __via_typed: #typed_path_ty = __via_chain.into();
            let __via_untyped: #toasty::core::stmt::Path = __via_typed.into();
            __via_untyped
        }
    }
}
