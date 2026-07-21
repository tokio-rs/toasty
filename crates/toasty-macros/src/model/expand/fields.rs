use super::{Expand, util};
use crate::model::schema::FieldTy::{BelongsTo, HasMany, HasOne, Primitive};
use crate::model::schema::ModelKind;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

const FIELD_STRUCT_RESERVED_METHODS: &[&str] =
    &["from_path", "path", "eq", "in_query", "into_root", "create"];

const FIELD_LIST_STRUCT_RESERVED_METHODS: &[&str] = &["from_path", "path", "any", "all", "create"];

impl Expand<'_> {
    pub(super) fn expand_field_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_struct_ident = self.field_struct_ident();
        let model_ident = &self.model.ident;
        let schema_trait = self.schema_trait();
        // Cloned so the field-method closure below can capture it by move while
        // `into_root` keeps using the original.
        let field_schema_trait = schema_trait.clone();

        let create_method = if let ModelKind::Root(root) = &self.model.kind {
            let create_struct_ident = &root.create_struct_ident;
            quote! {
                #vis fn create(&self) -> #create_struct_ident {
                    #create_struct_ident::default()
                }
            }
        } else {
            TokenStream::new()
        };

        // Generate methods that return field paths for the model
        let methods = self
            .model
            .fields
            .iter()
            .enumerate()
            .filter(|(_, field)| {
                !util::ident_is_reserved(&field.name.ident, FIELD_STRUCT_RESERVED_METHODS)
            })
            .map(move |(offset, field)| {
                let field_ident = &field.name.ident;
                let field_offset = util::int(offset);

                match &field.ty {
                    Primitive(ty) => {
                        // The accessor resolves its path through the field
                        // type's `Field` impl, so the type decides its own path
                        // shape: a struct embed (column-expanded or
                        // `#[document]`) yields a chainable Fields handle
                        // (`profile().name()`), a `Vec<_>` collection yields a
                        // list leaf.
                        self.expand_primitive_field_method(field_ident, ty, &field_offset)
                    }
                    BelongsTo(rel) => {
                        self.expand_one_relation_field_method(
                            field_ident,
                            quote!(#toasty::RelationOneField),
                            &rel.ty,
                            &field_offset,
                        )
                    }
                    HasOne(rel) => {
                        self.expand_one_relation_field_method(
                            field_ident,
                            quote!(#toasty::RelationOneField),
                            &rel.ty,
                            &field_offset,
                        )
                    }
                    HasMany(rel) => {
                        let ty = &rel.ty;
                        let span = field_ident.span();
                        let path = quote! {
                            self.path().chain(<#model_ident as #field_schema_trait>::path_field(#field_offset))
                        };

                        if rel.via.is_some() {
                            // A `via` step returns its terminal's path handle
                            // (`ViaTarget::Path`): a model terminal yields a
                            // chainable `ManyField`, so a scalar terminal can
                            // follow a via intermediate (`a.b.field` where `b` is
                            // a via); a scalar terminal yields a plain list path,
                            // a leaf. Both stay includable / selectable
                            // (`Into<stmt::Path>`). The element type comes from
                            // `ViaManyField`, which works for scalar terminals too
                            // (where there is no `RelationManyField::Target`).
                            quote_spanned! { span=>
                                #vis fn #field_ident(&self) -> #toasty::ViaPath<#ty, __Origin> {
                                    <<#ty as #toasty::ViaManyField>::Target as #toasty::ViaTarget>::new_path(#path)
                                }
                            }
                        } else {
                            quote_spanned! { span=>
                                #vis fn #field_ident(&self) -> <<#ty as #toasty::RelationManyField>::Target as #toasty::Model>::ManyField<__Origin> {
                                    <<<#ty as #toasty::RelationManyField>::Target as #toasty::Model>::ManyField<__Origin>>::from_path(#path)
                                }
                            }
                        }
                    }
                }
            });

        // Span the struct definition to the model ident so "method not found
        // for this struct" errors point at `struct User`, not at the derive.
        let model_span = model_ident.span();
        let struct_def = quote_spanned! { model_span=>
            #vis struct #field_struct_ident<__Origin> {
                path: #toasty::Path<__Origin, #model_ident>,
            }
        };

        let filter_method = self.expand_filter_method(quote!(#model_ident));

        quote!(
            #struct_def

            impl<__Origin> #field_struct_ident<__Origin> {
                #vis const fn from_path(path: #toasty::Path<__Origin, #model_ident>) -> #field_struct_ident<__Origin> {
                    #field_struct_ident { path }
                }

                fn path(&self) -> #toasty::Path<__Origin, #model_ident> {
                    self.path.clone()
                }

                #vis fn eq(self, rhs: impl #toasty::IntoExpr<#model_ident>) -> #toasty::stmt::Expr<bool> {
                    use #toasty::IntoExpr;
                    self.path.eq(rhs.into_expr())
                }

                #vis fn in_query(self, rhs: impl #toasty::IntoStatement<Returning = #toasty::List<#model_ident>>) -> #toasty::stmt::Expr<bool> {
                    self.path.in_query(rhs)
                }

                /// Discard `self`'s origin parameter and return a fresh
                /// fields struct typed against this model. Used by
                /// `update!` to build `stmt::patch` paths for embedded
                /// partial updates.
                #[doc(hidden)]
                pub fn into_root(self) -> #field_struct_ident<#model_ident> {
                    let _ = self;
                    #field_struct_ident::from_path(<#model_ident as #schema_trait>::path_root())
                }

                #filter_method

                #create_method

                #( #methods )*
            }

            impl<__Origin> Into<#toasty::Path<__Origin, #model_ident>> for #field_struct_ident<__Origin> {
                fn into(self) -> #toasty::Path<__Origin, #model_ident> {
                    self.path
                }
            }

            impl<__Origin> #toasty::IntoExpr<#model_ident> for #field_struct_ident<__Origin> {
                fn into_expr(self) -> #toasty::stmt::Expr<#model_ident> {
                    self.path.into_expr()
                }

                fn by_ref(&self) -> #toasty::stmt::Expr<#model_ident> {
                    self.path.by_ref()
                }
            }

            impl<__Origin> Into<#toasty::stmt::Include<__Origin, #model_ident>> for #field_struct_ident<__Origin> {
                fn into(self) -> #toasty::stmt::Include<__Origin, #model_ident> {
                    self.path.into()
                }
            }
        )
    }

    fn expand_filter_method(&self, target_ty: TokenStream) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        if !matches!(self.model.kind, ModelKind::Root(_)) {
            return TokenStream::new();
        }
        // A field named `filter` keeps its accessor; the include-filter
        // combinator is skipped for this model rather than colliding.
        if self
            .model
            .fields
            .iter()
            .any(|field| util::bare_ident_name(&field.name.ident) == "filter")
        {
            return TokenStream::new();
        }
        quote! {
            /// Restricts the related rows loaded by `.include(...)`.
            #vis fn filter(self, predicate: #toasty::stmt::Expr<bool>) -> #toasty::stmt::Include<__Origin, #target_ty> {
                #toasty::stmt::Include::from_path_and_query(
                    self.path,
                    #toasty::stmt::Query::<#toasty::stmt::List<#model_ident>>::all().filter(predicate),
                )
            }
        }
    }

    pub(super) fn expand_field_list_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_list_struct_ident = self.field_list_struct_ident();
        let model_ident = &self.model.ident;
        let is_root = matches!(self.model.kind, ModelKind::Root(_));

        // Generate methods that return list field paths.
        //
        // An embedded enum flattens every variant's fields into one list, so two
        // variants may declare the same field name — e.g. a column shared across
        // variants via `#[shared(name)]`. Emit each accessor name only once to
        // avoid duplicate method definitions. Root models and embedded structs
        // can never have duplicate field names, so this dedup is a no-op there.
        let mut seen_names = std::collections::HashSet::new();
        let methods = self
            .model
            .fields
            .iter()
            .enumerate()
            .filter(move |(_, field)| {
                seen_names.insert(field.name.as_str().to_string())
                    && !util::ident_is_reserved(
                        &field.name.ident,
                        FIELD_LIST_STRUCT_RESERVED_METHODS,
                    )
            })
            .map(move |(offset, field)| {
                let field_ident = &field.name.ident;
                let field_offset = util::int(offset);

                match &field.ty {
                    Primitive(_) if field.attrs.document.is_some() => TokenStream::new(),
                    Primitive(ty) => {
                        self.expand_list_primitive_field_method(field_ident, ty, &field_offset)
                    }
                    // All relations from a list context return the list variant
                    BelongsTo(rel) => {
                        let ty = &rel.ty;
                        self.expand_list_relation_field_method(
                            field_ident,
                            quote!(#toasty::RelationOneField),
                            ty,
                            &field_offset,
                        )
                    }
                    HasOne(rel) => {
                        let ty = &rel.ty;
                        self.expand_list_relation_field_method(
                            field_ident,
                            quote!(#toasty::RelationOneField),
                            ty,
                            &field_offset,
                        )
                    }
                    HasMany(rel) if rel.via.is_some() => {
                        // See the `via` branch in `expand_field_struct`: a via
                        // step returns its terminal's `ViaTarget::Path` handle —
                        // a chainable `ManyField` for a model terminal, a plain
                        // list path for a scalar terminal — and its element type
                        // comes from `ViaManyField` so scalar terminals work too.
                        let ty = &rel.ty;
                        let span = field_ident.span();
                        let schema_trait = self.schema_trait();
                        quote_spanned! { span=>
                            #vis fn #field_ident(&self) -> #toasty::ViaPath<#ty, __Origin> {
                                <<#ty as #toasty::ViaManyField>::Target as #toasty::ViaTarget>::new_path(
                                    self.path().chain(
                                        <#model_ident as #schema_trait>::path_field(#field_offset)
                                    )
                                )
                            }
                        }
                    }
                    HasMany(rel) => {
                        let ty = &rel.ty;
                        self.expand_list_relation_field_method(
                            field_ident,
                            quote!(#toasty::RelationManyField),
                            ty,
                            &field_offset,
                        )
                    }
                }
            });

        let create_method = if let ModelKind::Root(root) = &self.model.kind {
            let create_struct_ident = &root.create_struct_ident;
            quote! {
                #vis fn create(&self) -> #create_struct_ident {
                    #create_struct_ident::default()
                }
            }
        } else {
            TokenStream::new()
        };

        // any() / all() are only available on root models (they require the
        // `Model` trait bound).
        let any_method = if is_root {
            quote! {
                /// Filter the parent model by a condition on the associated
                /// (child) model. Returns `true` when **any** associated record
                /// satisfies `filter`.
                #vis fn any(self, filter: #toasty::stmt::Expr<bool>) -> #toasty::stmt::Expr<bool> {
                    self.path.any(filter)
                }

                /// Filter the parent model by a condition on the associated
                /// (child) model. Returns `true` when **all** associated records
                /// satisfy `filter` (vacuously true when there are no
                /// associated records).
                #vis fn all(self, filter: #toasty::stmt::Expr<bool>) -> #toasty::stmt::Expr<bool> {
                    self.path.all(filter)
                }
            }
        } else {
            TokenStream::new()
        };

        let model_span = model_ident.span();
        let struct_def = quote_spanned! { model_span=>
            #vis struct #field_list_struct_ident<__Origin> {
                path: #toasty::Path<__Origin, #toasty::List<#model_ident>>,
            }
        };

        let filter_method = self.expand_filter_method(quote!(#toasty::List<#model_ident>));

        quote!(
            #struct_def

            impl<__Origin> #field_list_struct_ident<__Origin> {
                #vis const fn from_path(path: #toasty::Path<__Origin, #toasty::List<#model_ident>>) -> #field_list_struct_ident<__Origin> {
                    #field_list_struct_ident { path }
                }

                fn path(&self) -> #toasty::Path<__Origin, #toasty::List<#model_ident>> {
                    self.path.clone()
                }

                #any_method

                #filter_method

                #create_method

                #( #methods )*
            }

            impl<__Origin> Into<#toasty::Path<__Origin, #toasty::List<#model_ident>>> for #field_list_struct_ident<__Origin> {
                fn into(self) -> #toasty::Path<__Origin, #toasty::List<#model_ident>> {
                    self.path
                }
            }

            impl<__Origin> #toasty::IntoExpr<#toasty::List<#model_ident>> for #field_list_struct_ident<__Origin> {
                fn into_expr(self) -> #toasty::stmt::Expr<#toasty::List<#model_ident>> {
                    self.path.into_expr()
                }

                fn by_ref(&self) -> #toasty::stmt::Expr<#toasty::List<#model_ident>> {
                    self.path.by_ref()
                }
            }

            impl<__Origin> Into<#toasty::stmt::Include<__Origin, #toasty::List<#model_ident>>> for #field_list_struct_ident<__Origin> {
                fn into(self) -> #toasty::stmt::Include<__Origin, #toasty::List<#model_ident>> {
                    self.path.into()
                }
            }
        )
    }

    pub(super) fn expand_model_field_struct_init(&self) -> TokenStream {
        let vis = &self.model.vis;
        let field_struct_ident = self.field_struct_ident();
        let model_ident = &self.model.ident;
        let schema_trait = self.schema_trait();

        let doc_fields = self.doc_fields();

        // Generate fields() as a method instead of const to avoid const initialization issues
        // This will be placed inside the existing impl block for the model
        quote!(
            #[doc = #doc_fields]
            #vis fn fields() -> #field_struct_ident<#model_ident> {
                #field_struct_ident {
                    path: <#model_ident as #schema_trait>::path_root(),
                }
            }
        )
    }

    fn field_struct_ident(&self) -> &syn::Ident {
        use crate::model::schema::ModelKind;

        match &self.model.kind {
            ModelKind::Root(root) => &root.field_struct_ident,
            ModelKind::EmbeddedStruct(embedded) => &embedded.field_struct_ident,
            ModelKind::EmbeddedEnum(e) => &e.field_struct_ident,
        }
    }

    /// The schema trait (`Model` or `Embed`) implemented by the type being
    /// expanded. The `path_root` / `path_field` constructors live on both, so
    /// generated field accessors dispatch through whichever one this type
    /// implements.
    pub(super) fn schema_trait(&self) -> TokenStream {
        use crate::model::schema::ModelKind;

        let toasty = &self.toasty;
        match &self.model.kind {
            ModelKind::Root(_) => quote!(#toasty::Model),
            ModelKind::EmbeddedStruct(_) | ModelKind::EmbeddedEnum(_) => quote!(#toasty::Embed),
        }
    }

    pub(super) fn field_list_struct_ident(&self) -> &syn::Ident {
        use crate::model::schema::ModelKind;

        match &self.model.kind {
            ModelKind::Root(root) => &root.field_list_struct_ident,
            ModelKind::EmbeddedStruct(embedded) => &embedded.field_list_struct_ident,
            ModelKind::EmbeddedEnum(e) => &e.field_list_struct_ident,
        }
    }

    pub(super) fn expand_field_name_to_id(&self) -> TokenStream {
        let toasty = &self.toasty;

        let fields = self
            .model
            .fields
            .iter()
            .enumerate()
            .map(move |(offset, field)| {
                let field_name = field.name.as_str();
                let field_offset = util::int(offset);

                quote!( #field_name => #toasty::core::schema::app::FieldId { model: Self::id(), index: #field_offset }, )
            });

        quote! {
            fn field_name_to_id(name: &str) -> #toasty::core::schema::app::FieldId {
                use #toasty::Model;

                match name {
                    #( #fields )*
                    _ => todo!("field_name_to_id: {}", name),
                }
            }
        }
    }

    /// Generates a field accessor method for a primitive field on the list
    /// fields struct, using `Field::new_list_path`.
    fn expand_list_primitive_field_method(
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

        quote_spanned! { span=>
            #vis fn #field_ident(&self) -> <#ty as #toasty::Field>::ListPath<__Origin> {
                <#ty as #toasty::Field>::new_list_path(
                    self.path().chain(
                        <#model_ident as #schema_trait>::path_field(#field_offset)
                    )
                )
            }
        }
    }

    /// Generates a relation accessor method on the list fields struct.
    /// All relations from a list context return the ManyField (list) variant.
    fn expand_list_relation_field_method(
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
            #vis fn #field_ident(&self) -> <<#ty as #field_trait>::Target as #toasty::Model>::ManyField<__Origin> {
                <<<#ty as #field_trait>::Target as #toasty::Model>::ManyField<__Origin>>::from_path(
                    self.path().chain(
                        <#model_ident as #schema_trait>::path_field(#field_offset)
                    )
                )
            }
        }
    }
}
