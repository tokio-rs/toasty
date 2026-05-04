use super::{Expand, util};
use crate::model::schema::FieldTy::{BelongsTo, HasMany, HasOne, Primitive};
use crate::model::schema::ModelKind;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

impl Expand<'_> {
    /// Generate `ValidateCreate` impls for the field struct and field list struct.
    ///
    /// Only generated for root models since `ValidateCreate` references
    /// `<Model>::CREATE_META` which is only available on root models.
    pub(super) fn expand_validate_create_impls(&self) -> TokenStream {
        let ModelKind::Root(_) = &self.model.kind else {
            return TokenStream::new();
        };

        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        let field_struct_ident = self.field_struct_ident();
        let field_list_struct_ident = self.field_list_struct_ident();

        quote! {
            #[diagnostic::do_not_recommend]
            impl<__Origin> #toasty::ValidateCreate for #field_struct_ident<__Origin> {
                const CREATE_META: &'static #toasty::CreateMeta =
                    &<#model_ident as #toasty::Model>::CREATE_META;
            }

            #[diagnostic::do_not_recommend]
            impl<__Origin> #toasty::ValidateCreate for #field_list_struct_ident<__Origin> {
                const CREATE_META: &'static #toasty::CreateMeta =
                    &<#model_ident as #toasty::Model>::CREATE_META;
            }
        }
    }
}

impl Expand<'_> {
    pub(super) fn expand_field_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_struct_ident = self.field_struct_ident();
        let model_ident = &self.model.ident;

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
            .map(move |(offset, field)| {
                let field_ident = &field.name.ident;
                let field_offset = util::int(offset);

                match &field.ty {
                    Primitive(_) if field.attrs.serialize.is_some() => {
                        // Serialized fields are stored as opaque JSON; no field accessor
                        TokenStream::new()
                    }
                    Primitive(ty) if field.attrs.deferred => {
                        let inner: syn::Type =
                            syn::parse_quote!(<#ty as #toasty::Defer>::Inner);
                        self.expand_primitive_field_method(field_ident, &inner, &field_offset)
                    }
                    Primitive(ty) => {
                        self.expand_primitive_field_method(field_ident, ty, &field_offset)
                    }
                    BelongsTo(rel) => {
                        self.expand_one_relation_field_method(field_ident, &rel.ty, &field_offset)
                    }
                    HasOne(rel) => {
                        self.expand_one_relation_field_method(field_ident, &rel.ty, &field_offset)
                    }
                    HasMany(rel) => {
                        let ty = &rel.ty;
                        let span = field_ident.span();
                        let path = quote! {
                            self.path().chain(#toasty::Path::<#model_ident, _>::from_field_index(#field_offset))
                        };

                        quote_spanned! { span=>
                            #vis fn #field_ident(&self) -> <#ty as #toasty::Relation>::ManyField<__Origin> {
                                <#ty as #toasty::Relation>::ManyField::from_path(#path)
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

                #create_method

                #( #methods )*
            }

            impl<__Origin> Into<#toasty::Path<__Origin, #model_ident>> for #field_struct_ident<__Origin> {
                fn into(self) -> #toasty::Path<__Origin, #model_ident> {
                    self.path
                }
            }
        )
    }

    pub(super) fn expand_field_list_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_list_struct_ident = self.field_list_struct_ident();
        let model_ident = &self.model.ident;
        let is_root = matches!(self.model.kind, ModelKind::Root(_));

        // Generate methods that return list field paths
        let methods = self
            .model
            .fields
            .iter()
            .enumerate()
            .map(move |(offset, field)| {
                let field_ident = &field.name.ident;
                let field_offset = util::int(offset);

                match &field.ty {
                    Primitive(_) if field.attrs.serialize.is_some() => TokenStream::new(),
                    Primitive(ty) if field.attrs.deferred => {
                        let inner: syn::Type = syn::parse_quote!(<#ty as #toasty::Defer>::Inner);
                        self.expand_list_primitive_field_method(field_ident, &inner, &field_offset)
                    }
                    Primitive(ty) => {
                        self.expand_list_primitive_field_method(field_ident, ty, &field_offset)
                    }
                    // All relations from a list context return the list variant
                    BelongsTo(rel) => {
                        let ty = &rel.ty;
                        self.expand_list_relation_field_method(field_ident, ty, &field_offset)
                    }
                    HasOne(rel) => {
                        let ty = &rel.ty;
                        self.expand_list_relation_field_method(field_ident, ty, &field_offset)
                    }
                    HasMany(rel) => {
                        let ty = &rel.ty;
                        self.expand_list_relation_field_method(field_ident, ty, &field_offset)
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

        // any() / all() are only available on root models (requires Model trait bound)
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

                #create_method

                #( #methods )*
            }

            impl<__Origin> Into<#toasty::Path<__Origin, #toasty::List<#model_ident>>> for #field_list_struct_ident<__Origin> {
                fn into(self) -> #toasty::Path<__Origin, #toasty::List<#model_ident>> {
                    self.path
                }
            }
        )
    }

    pub(super) fn expand_model_field_struct_init(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_struct_ident = self.field_struct_ident();
        let model_ident = &self.model.ident;

        // Generate fields() as a method instead of const to avoid const initialization issues
        // This will be placed inside the existing impl block for the model
        quote!(
            #vis fn fields() -> #field_struct_ident<#model_ident> {
                #field_struct_ident {
                    path: #toasty::Path::root(),
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
                use #toasty::{Model, Register};

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
        let span = field_ident.span();

        quote_spanned! { span=>
            #vis fn #field_ident(&self) -> <#ty as #toasty::Field>::ListPath<__Origin> {
                <#ty as #toasty::Field>::new_list_path(
                    self.path().chain(
                        #toasty::Path::<#model_ident, _>::from_field_index(#field_offset)
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
        ty: &syn::Type,
        field_offset: &TokenStream,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let span = field_ident.span();

        quote_spanned! { span=>
            #vis fn #field_ident(&self) -> <#ty as #toasty::Relation>::ManyField<__Origin> {
                <#ty as #toasty::Relation>::ManyField::from_path(
                    self.path().chain(
                        #toasty::Path::<#model_ident, _>::from_field_index(#field_offset)
                    )
                )
            }
        }
    }
}
