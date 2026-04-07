use super::{Expand, util};
use crate::model::schema::FieldTy::{BelongsTo, HasMany, HasOne, Primitive};
use crate::model::schema::ModelKind;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

impl Expand<'_> {
    pub(super) fn expand_field_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let field_struct_ident = self.field_struct_ident();
        let model_ident = &self.model.ident;

        let doc_struct = format!(
            "Typed field paths for [`{model_name}`].\n\
             \n\
             Returned by [`{model_name}::fields()`]. Use the accessor methods\n\
             to build filter expressions and navigate to related models.\n\
             \n\
             See the [Toasty guide](https://docs.rs/toasty/latest/toasty/) for\n\
             examples of building queries with field paths.",
            model_name = model_ident,
        );
        let doc_eq = format!(
            "Return a filter expression that matches [`{model_name}`] records\n\
             equal to `rhs` (compared by primary key).",
            model_name = model_ident,
        );
        let doc_in_query = format!(
            "Return a filter expression that matches [`{model_name}`] records\n\
             whose primary key appears in the result set of `rhs`.",
            model_name = model_ident,
        );

        let create_method = if let ModelKind::Root(root) = &self.model.kind {
            let create_struct_ident = &root.create_struct_ident;
            let doc_create = format!(
                "Return a new create builder for [`{model_name}`].",
                model_name = model_ident,
            );
            quote! {
                #[doc = #doc_create]
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

                        let doc = format!(
                            "Access the `{field}` has-many relation path.",
                            field = field_ident,
                        );

                        quote_spanned! { span=>
                            #[doc = #doc]
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
            #[doc = #doc_struct]
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

                #[doc = #doc_eq]
                #vis fn eq(self, rhs: impl #toasty::IntoExpr<#model_ident>) -> #toasty::stmt::Expr<bool> {
                    use #toasty::IntoExpr;
                    self.path.eq(rhs.into_expr())
                }

                #[doc = #doc_in_query]
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

        let doc_list_struct = format!(
            "Typed field paths for a list of [`{model_name}`] records.\n\
             \n\
             Used when navigating from a has-many association to build\n\
             sub-filters and access nested fields.",
            model_name = model_ident,
        );

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
            let doc_create = format!(
                "Return a new create builder for [`{model_name}`].",
                model_name = model_ident,
            );
            quote! {
                #[doc = #doc_create]
                #vis fn create(&self) -> #create_struct_ident {
                    #create_struct_ident::default()
                }
            }
        } else {
            TokenStream::new()
        };

        // any() is only available on root models (requires Model trait bound)
        let doc_any = format!(
            "Return a filter expression that is `true` when **any** associated\n\
             [`{model_name}`] record satisfies `filter`.\n\
             \n\
             Use this to filter a parent model by a condition on its children.",
            model_name = model_ident,
        );
        let any_method = if is_root {
            quote! {
                #[doc = #doc_any]
                #vis fn any(self, filter: #toasty::stmt::Expr<bool>) -> #toasty::stmt::Expr<bool> {
                    self.path.any(filter)
                }
            }
        } else {
            TokenStream::new()
        };

        let model_span = model_ident.span();
        let struct_def = quote_spanned! { model_span=>
            #[doc = #doc_list_struct]
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

        let doc_fields = format!(
            "Return typed field paths for building filter expressions.\n\
             \n\
             Each accessor on the returned [`{fields_name}`] corresponds to a\n\
             field on [`{model_name}`] and can be used to build comparisons,\n\
             e.g. `{model_name}::fields().{example}.eq(value)`.",
            model_name = model_ident,
            fields_name = field_struct_ident,
            example = self
                .model
                .fields
                .first()
                .map(|f| f.name.ident.to_string())
                .unwrap_or_else(|| "field_name".to_string()),
        );

        // Generate fields() as a method instead of const to avoid const initialization issues
        // This will be placed inside the existing impl block for the model
        quote!(
            #[doc = #doc_fields]
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
                let field_name = field.name.ident.to_string();
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

        let doc = format!(
            "Access the `{field}` field path within this list context.",
            field = field_ident,
        );

        quote_spanned! { span=>
            #[doc = #doc]
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

        let doc = format!(
            "Access the `{field}` relation path within this list context.",
            field = field_ident,
        );

        quote_spanned! { span=>
            #[doc = #doc]
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
