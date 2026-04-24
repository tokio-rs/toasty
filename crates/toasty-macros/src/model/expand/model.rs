use super::{Expand, util};
use crate::model::schema::{FieldTy, ModelKind};

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_impls(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;

        let (
            field_struct_ident,
            field_list_struct_ident,
            query_struct_ident,
            create_struct_ident,
            update_struct_ident,
        ) = match &self.model.kind {
            ModelKind::Root(root) => (
                &root.field_struct_ident,
                &root.field_list_struct_ident,
                &root.query_struct_ident,
                &root.create_struct_ident,
                &root.update_struct_ident,
            ),
            ModelKind::EmbeddedStruct(_) | ModelKind::EmbeddedEnum(_) => {
                // Embedded models don't generate CRUD methods, just return early
                return TokenStream::new();
            }
        };
        let model_schema = self.expand_model_schema();
        let field_register_calls = self.expand_field_register_calls();
        let model_fields = self.expand_model_field_struct_init();
        let load_body = self.expand_load_body(true);
        let filter_methods = self.expand_model_filter_methods();
        let field_name_to_id = self.expand_field_name_to_id();
        let relation_methods = self.expand_model_relation_methods();
        let into_statement_body = self.expand_model_into_statement_body();
        let into_delete_body = self.expand_model_into_delete_body();
        let into_expr_body_ref = self.expand_model_into_expr_body(true);
        let into_expr_body_val = self.expand_model_into_expr_body(false);
        let reload_trait_method = self.expand_reload_trait_method();
        let version_update_stmts = self.expand_version_update_stmts();

        quote! {
            impl #model_ident {
                #model_fields
                #filter_methods
                #relation_methods

                #vis fn create() -> #create_struct_ident {
                    #create_struct_ident::default()
                }

                #vis fn create_many() -> #toasty::stmt::CreateMany<#model_ident> {
                    #toasty::stmt::CreateMany::default()
                }

                #vis fn update(&mut self) -> #update_struct_ident<&mut Self> {
                    let mut s = #update_struct_ident {
                        assignments: #toasty::core::stmt::Assignments::default(),
                        condition: None,
                        target: self,
                    };
                    s.apply_update_defaults();
                    #version_update_stmts
                    s
                }

                #vis fn all() -> #query_struct_ident {
                    #query_struct_ident::default()
                }

                #vis fn filter(expr: #toasty::stmt::Expr<bool>) -> #query_struct_ident {
                    #query_struct_ident::from_stmt(#toasty::stmt::Query::filter(expr))
                }

                #vis fn delete(self) -> #toasty::stmt::Delete<()> {
                    #into_delete_body
                }
            }

            impl #toasty::Register for #model_ident {
                fn id() -> #toasty::core::schema::app::ModelId {
                    static ID: std::sync::OnceLock<#toasty::core::schema::app::ModelId> = std::sync::OnceLock::new();
                    *ID.get_or_init(|| #toasty::generate_unique_id())
                }

                #model_schema

                fn register(model_set: &mut #toasty::core::schema::app::ModelSet) {
                    if model_set.contains(Self::id()) {
                        return;
                    }
                    model_set.add(Self::schema());
                    #( #field_register_calls )*
                }
            }

            #toasty::inventory::submit! {
                #toasty::DiscoverItem::new(
                    env!("CARGO_PKG_NAME"),
                    |model_set| { <#model_ident as #toasty::Register>::register(model_set); },
                )
            }

            impl #toasty::Load for #model_ident {
                type Output = Self;

                fn ty() -> #toasty::core::stmt::Type {
                    #toasty::core::stmt::Type::Model(<Self as #toasty::Register>::id())
                }

                fn load(value: #toasty::core::stmt::Value) -> #toasty::Result<Self> {
                    #load_body
                }

                #reload_trait_method
            }

            impl #toasty::Model for #model_ident {
                type Query = #query_struct_ident;
                type Create = #create_struct_ident;
                type Update<'a> = #update_struct_ident<&'a mut Self>;
                type UpdateQuery = #update_struct_ident;
                type Path<__Origin> = #field_struct_ident<__Origin>;

                fn new_path<__Origin>(path: #toasty::Path<__Origin, Self>) -> Self::Path<__Origin> {
                    #field_struct_ident::from_path(path)
                }
            }

            impl #toasty::Relation for #model_ident {
                type Model = #model_ident;
                type Expr = #model_ident;
                type Query = #query_struct_ident;
                type Create = #create_struct_ident;
                type Many = Many;
                type ManyField<__Origin> = #field_list_struct_ident<__Origin>;
                type One = One;
                type OneField<__Origin> = #field_struct_ident<__Origin>;
                type OptionOne = OptionOne;

                fn new_many_field<__Origin>(
                    path: #toasty::Path<__Origin, #toasty::List<Self::Model>>,
                ) -> #field_list_struct_ident<__Origin> {
                    #field_list_struct_ident::from_path(path)
                }

                #field_name_to_id
            }

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

            impl #toasty::IntoStatement for &#model_ident {
                type Returning = #model_ident;

                fn into_statement(self) -> #toasty::Statement<#model_ident> {
                    use #toasty::IntoStatement;
                    #into_statement_body
                }
            }

            impl #toasty::IntoStatement for &mut #model_ident {
                type Returning = #model_ident;

                fn into_statement(self) -> #toasty::Statement<#model_ident> {
                    (&*self).into_statement()
                }
            }

            impl #toasty::IntoStatement for #model_ident {
                type Returning = #model_ident;

                fn into_statement(self) -> #toasty::Statement<#model_ident> {
                    (&self).into_statement()
                }
            }
        }
    }

    pub(super) fn expand_embedded_model_impls(&self) -> TokenStream {
        let model_ident = &self.model.ident;
        let model_fields = self.expand_model_field_struct_init();

        quote! {
            impl #model_ident {
                #model_fields
            }
        }
    }

    fn expand_model_into_statement_body(&self) -> TokenStream {
        let toasty = &self.toasty;
        let filter = self.primary_key_filter();
        let query_struct_ident = &self.model.kind.as_root_unwrap().query_struct_ident;
        let filter_method_ident = &filter.filter_method_ident;
        let arg_idents = self.expand_filter_arg_idents(filter);

        quote! {
            #toasty::IntoStatement::into_statement(
                #query_struct_ident::default()
                    .#filter_method_ident( #( & self.#arg_idents ),* )
                    .one()
            )
        }
    }

    fn expand_model_into_delete_body(&self) -> TokenStream {
        let filter = self.primary_key_filter();
        let query_struct_ident = &self.model.kind.as_root_unwrap().query_struct_ident;
        let filter_method_ident = &filter.filter_method_ident;
        let arg_idents = self.expand_filter_arg_idents(filter);

        let version_condition = self.expand_version_delete_condition();

        quote! {
            {
                let __delete = #query_struct_ident::default()
                    .#filter_method_ident( #( & self.#arg_idents ),* )
                    .delete();
                #version_condition
            }
        }
    }

    /// Generate the version condition to set on the delete statement, if the
    /// model has a `#[version]` field. Returns a block that evaluates to
    /// `toasty::stmt::Delete<()>`.
    fn expand_version_delete_condition(&self) -> TokenStream {
        let toasty = &self.toasty;

        let Some(version_index) = self.model.kind.as_root().and_then(|r| r.version_field) else {
            return quote! { __delete };
        };

        let field = &self.model.fields[version_index];
        let index_tokenized = util::int(version_index);
        let field_ident = &field.name.ident;

        quote! {
            __delete.set_condition(
                #toasty::core::stmt::Condition::new(
                    #toasty::core::stmt::Expr::eq(
                        #toasty::core::stmt::Expr::Reference(
                            #toasty::core::stmt::ExprReference::Field {
                                nesting: 0,
                                index: #index_tokenized,
                            }
                        ),
                        #toasty::core::stmt::Expr::Value(
                            #toasty::core::stmt::Value::U64(self.#field_ident)
                        ),
                    )
                )
            )
        }
    }

    pub(super) fn expand_model_into_expr_body(&self, by_ref: bool) -> TokenStream {
        let toasty = &self.toasty;

        let pk_fields: Vec<_> = self
            .model
            .primary_key_fields()
            .expect("into_expr called on model without primary key")
            .collect();

        if pk_fields.len() == 1 {
            let expr = pk_fields.iter().map(|field| {
                let field_ident = &field.name.ident;
                let ty = match &field.ty {
                    FieldTy::Primitive(ty) => ty,
                    _ => todo!(),
                };

                let into_expr = if by_ref {
                    quote!((&self.#field_ident))
                } else {
                    quote!(self.#field_ident)
                };

                quote! {
                    let expr: #toasty::stmt::Expr<#ty> = #toasty::IntoExpr::into_expr(#into_expr);
                    expr.cast()
                }
            });

            quote!( #( #expr )* )
        } else {
            let expr = pk_fields
                .iter()
                .map(|field| {
                    let field_ident = &field.name.ident;
                    let amp = if by_ref { quote!(&) } else { quote!() };
                    quote!( #amp self.#field_ident)
                })
                .collect::<Vec<_>>();

            let ty = pk_fields
                .iter()
                .map(|field| match &field.ty {
                    FieldTy::Primitive(ty) => ty,
                    _ => todo!(),
                })
                .collect::<Vec<_>>();

            quote! {
                let expr: #toasty::stmt::Expr<( #( #ty ),* )> =
                    #toasty::IntoExpr::into_expr(( #( #expr ),* ));
                expr.cast()
            }
        }
    }

    pub(super) fn expand_embedded_into_expr_body(
        &self,
        fields_named: bool,
        by_ref: bool,
    ) -> TokenStream {
        let toasty = &self.toasty;

        // For embedded types, create a record expression from all fields
        // Currently only primitive fields are supported in embedded types
        let field_exprs = self.model.fields.iter().enumerate().map(|(index, field)| {
            let ty = match &field.ty {
                FieldTy::Primitive(ty) => ty,
                _ => panic!("only primitive fields are supported in embedded types"),
            };

            let value = if fields_named {
                let field_ident = &field.name.ident;
                if by_ref {
                    quote!((&self.#field_ident))
                } else {
                    quote!(self.#field_ident)
                }
            } else {
                let idx = syn::Index::from(index);
                if by_ref {
                    quote!((&self.#idx))
                } else {
                    quote!(self.#idx)
                }
            };

            self.expand_into_untyped_expr(ty, value)
        });

        quote! {
            #toasty::stmt::Expr::from_untyped(
                #toasty::core::stmt::Expr::record([
                    #( #field_exprs ),*
                ])
            )
        }
    }

    /// Generates the body for loading a model or embedded type from a Value.
    ///
    /// This method is used by both:
    /// - Root models (in `Load::load`) - supports all field types
    /// - Embedded types (in `Load::load`) - only primitive fields
    ///
    /// The generated code pattern matches on `Value::Record`, extracts fields,
    /// and constructs the struct.
    pub(super) fn expand_load_body(&self, fields_named: bool) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;

        // Generate field loading expressions
        let field_loads = self.model.fields.iter().enumerate().map(|(index, field)| {
            let field_ident = &field.name.ident;
            let index_tokenized = util::int(index);
            let field_name_str = field.name.ident.to_string();

            let field_name = if fields_named {
                quote!(#field_ident:)
            } else {
                quote!()
            };

            match &field.ty {
                FieldTy::Primitive(_ty) if field.attrs.serialize.is_some() => {
                    let serialize_attr = field.attrs.serialize.as_ref().unwrap();

                    let json_deserialize = quote! {
                        let json_str = <String as #toasty::Load>::load(value)?;
                        #toasty::serde_json::from_str(&json_str)
                            .map_err(|e| #toasty::Error::from_args(
                                format_args!("failed to deserialize field '{}': {}", #field_name_str, e)
                            ))?
                    };

                    let field_value = if serialize_attr.nullable {
                        quote! {
                            if value.is_null() { None } else { Some({ #json_deserialize }) }
                        }
                    } else {
                        json_deserialize
                    };

                    quote! {
                        #field_name {
                            let value = record[#index_tokenized].take();
                            #field_value
                        },
                    }
                }
                FieldTy::Primitive(ty) => {
                    quote!(#field_name <#ty as #toasty::Load>::load(record[#index_tokenized].take())?,)
                }
                FieldTy::BelongsTo(_) => {
                    quote!(#field_name #toasty::BelongsTo::load(record[#index].take())?,)
                }
                FieldTy::HasMany(_) => {
                    quote!(#field_name #toasty::HasMany::load(record[#index].take())?,)
                }
                FieldTy::HasOne(_) => {
                    quote!(#field_name #toasty::HasOne::load(record[#index].take())?,)
                }
            }
        });

        let model_load = if fields_named {
            quote!(#model_ident {
                #( #field_loads )*
            })
        } else {
            quote!(#model_ident( #( #field_loads )* ))
        };

        quote! {
            match value {
                #toasty::core::stmt::Value::Null => {
                    Err(#toasty::Error::record_not_found(stringify!(#model_ident)))
                }
                #toasty::core::stmt::Value::Record(mut record) => {
                    Ok(#model_load)
                }
                value => Err(#toasty::Error::type_conversion(value, stringify!(#model_ident))),
            }
        }
    }

    /// Generate the body of the `reload` method for an embedded model's `Primitive` impl.
    ///
    /// Handles `SparseRecord` values (partial updates) by reloading only the specified
    /// sub-fields, and falls back to full `load` for complete record values.
    pub(super) fn expand_embedded_reload_body(&self, fields_named: bool) -> TokenStream {
        let toasty = &self.toasty;

        let reload_arms = self.model.fields.iter().enumerate().map(|(index, field)| {
            let i = util::int(index);
            let field_name_str = field.name.ident.to_string();

            // For newtypes, access via tuple index (target.0); otherwise by name
            let field_access = if fields_named {
                let field_ident = &field.name.ident;
                quote!(target.#field_ident)
            } else {
                let idx = syn::Index::from(index);
                quote!(target.#idx)
            };

            match &field.ty {
                FieldTy::Primitive(_ty) if field.attrs.serialize.is_some() => {
                    let serialize_attr = field.attrs.serialize.as_ref().unwrap();

                    let json_deserialize = quote! {
                        let json_str = <String as #toasty::Load>::load(value)?;
                        #toasty::serde_json::from_str(&json_str)
                            .map_err(|e| #toasty::Error::from_args(
                                format_args!("failed to deserialize field '{}': {}", #field_name_str, e)
                            ))?
                    };

                    let assign = if serialize_attr.nullable {
                        quote! {
                            if value.is_null() { None } else { Some({ #json_deserialize }) }
                        }
                    } else {
                        quote! { { #json_deserialize } }
                    };

                    quote! {
                        #i => {
                            #field_access = #assign;
                        }
                    }
                }
                FieldTy::Primitive(ty) => {
                    quote!(#i => <#ty as #toasty::Load>::reload(&mut #field_access, value)?,)
                }
                _ => {
                    quote!(#i => #field_access.unload(),)
                }
            }
        });

        quote! {
            match value {
                #toasty::core::stmt::Value::SparseRecord(sparse) => {
                    for (field, value) in sparse.into_iter() {
                        match field {
                            #( #reload_arms )*
                            _ => todo!("handle unknown field in embedded reload"),
                        }
                    }
                    Ok(())
                }
                value => {
                    *target = <Self as #toasty::Load>::load(value)?;
                    Ok(())
                }
            }
        }
    }
}
