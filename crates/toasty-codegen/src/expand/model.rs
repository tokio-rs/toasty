use super::{util, Expand};
use crate::schema::{FieldTy, ModelKind};

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_impls(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;

        let (query_struct_ident, create_struct_ident, update_struct_ident) = match &self.model.kind
        {
            ModelKind::Root(root) => (
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
        let model_fields = self.expand_model_field_struct_init();
        let load_body = self.expand_load_body();
        let filter_methods = self.expand_model_filter_methods();
        let field_name_to_id = self.expand_field_name_to_id();
        let relation_methods = self.expand_model_relation_methods();
        let into_statement_body = self.expand_model_into_statement_body();
        let into_expr_body_ref = self.expand_model_into_expr_body(true);
        let into_expr_body_val = self.expand_model_into_expr_body(false);
        let reload_method = self.expand_reload_method();

        quote! {
            impl #model_ident {
                #model_fields
                #filter_methods
                #relation_methods
                #reload_method

                #vis fn create() -> #create_struct_ident {
                    #create_struct_ident::default()
                }

                #vis fn create_many() -> #toasty::CreateMany<#model_ident> {
                    #toasty::CreateMany::default()
                }

                #vis fn update(&mut self) -> #update_struct_ident<&mut Self> {
                    use #toasty::IntoStatement;
                    let mut s = #update_struct_ident {
                        stmt: #toasty::stmt::Update::new_single((&*self).into_statement().into_list_query().unwrap()),
                        target: self,
                    };
                    s.apply_update_defaults();
                    s
                }

                #vis fn all() -> #query_struct_ident {
                    #query_struct_ident::default()
                }

                #vis fn filter(expr: #toasty::stmt::Expr<bool>) -> #query_struct_ident {
                    #query_struct_ident::from_stmt(#toasty::stmt::Query::filter(expr))
                }

                #vis fn delete(self) -> #toasty::stmt::Delete<#model_ident> {
                    use #toasty::IntoStatement;
                    self.into_statement().into_list_query().unwrap().delete().cast()
                }
            }

            impl #toasty::Register for #model_ident {
                fn id() -> #toasty::core::schema::app::ModelId {
                    static ID: std::sync::OnceLock<#toasty::core::schema::app::ModelId> = std::sync::OnceLock::new();
                    *ID.get_or_init(|| #toasty::generate_unique_id())
                }

                #model_schema
            }

            impl #toasty::Load for #model_ident {
                type Output = Self;
                fn load(value: #toasty::core::stmt::Value) -> #toasty::Result<Self> {
                    #load_body
                }
            }

            impl #toasty::Model for #model_ident {
                type Query = #query_struct_ident;
                type Create = #create_struct_ident;
                type Update<'a> = #update_struct_ident<&'a mut Self>;
                type UpdateQuery = #update_struct_ident;
            }

            impl #toasty::Relation for #model_ident {
                type Model = #model_ident;
                type Create = #create_struct_ident;
                type Expr = #model_ident;
                type Query = #query_struct_ident;
                type Many = Many;
                type ManyField<__Origin> = ManyField<__Origin>;
                type One = One;
                type OneField<__Origin> = OneField<__Origin>;
                type OptionOne = OptionOne;

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

            impl #toasty::stmt::IntoExpr<#toasty::List<#model_ident>> for #model_ident {
                fn into_expr(self) -> #toasty::stmt::Expr<#toasty::List<#model_ident>> {
                    #toasty::stmt::Expr::list([self])
                }

                fn by_ref(&self) -> #toasty::stmt::Expr<#toasty::List<#model_ident>> {
                    #toasty::stmt::Expr::list([self])
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
        let filter = self.primary_key_filter();
        let query_struct_ident = &self.model.kind.expect_root().query_struct_ident;
        let filter_method_ident = &filter.filter_method_ident;
        let arg_idents = self.expand_filter_arg_idents(filter);

        quote! {
            #query_struct_ident::default()
                .#filter_method_ident( #( & self.#arg_idents ),* )
                .into_statement()
                .cast()
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

    pub(super) fn expand_embedded_into_expr_body(&self, by_ref: bool) -> TokenStream {
        let toasty = &self.toasty;

        // For embedded types, create a record expression from all fields
        // Currently only primitive fields are supported in embedded types
        let field_exprs = self.model.fields.iter().map(|field| {
            let field_ident = &field.name.ident;
            let ty = match &field.ty {
                FieldTy::Primitive(ty) => ty,
                _ => panic!("only primitive fields are supported in embedded types"),
            };

            let value = if by_ref {
                quote!((&self.#field_ident))
            } else {
                quote!(self.#field_ident)
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
    /// - Root models (in `Model::load`) - supports all field types
    /// - Embedded types (in `Field::load`) - only primitive fields
    ///
    /// The generated code pattern matches on `Value::Record`, extracts fields,
    /// and constructs the struct.
    pub(super) fn expand_load_body(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;

        // Generate field loading expressions
        let field_loads = self.model.fields.iter().enumerate().map(|(index, field)| {
            let field_ident = &field.name.ident;
            let index_tokenized = util::int(index);
            let field_name_str = field.name.ident.to_string();

            match &field.ty {
                FieldTy::Primitive(_ty) if field.attrs.serialize.is_some() => {
                    let serialize_attr = field.attrs.serialize.as_ref().unwrap();

                    let json_deserialize = quote! {
                        let json_str = <String as #toasty::Field>::load(value)?;
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
                        #field_ident: {
                            let value = record[#index_tokenized].take();
                            #field_value
                        },
                    }
                }
                FieldTy::Primitive(ty) => {
                    quote!(#field_ident: <#ty as #toasty::Field>::load(record[#index_tokenized].take())?,)
                }
                FieldTy::BelongsTo(_) => {
                    quote!(#field_ident: #toasty::BelongsTo::load(record[#index].take())?,)
                }
                FieldTy::HasMany(_) => {
                    quote!(#field_ident: #toasty::HasMany::load(record[#index].take())?,)
                }
                FieldTy::HasOne(_) => {
                    quote!(#field_ident: #toasty::HasOne::load(record[#index].take())?,)
                }
            }
        });

        quote! {
            match value {
                #toasty::core::stmt::Value::Record(mut record) => {
                    Ok(#model_ident {
                        #( #field_loads )*
                    })
                }
                value => Err(#toasty::Error::type_conversion(value, stringify!(#model_ident))),
            }
        }
    }

    /// Generate the body of the `reload` method for an embedded model's `Primitive` impl.
    ///
    /// Handles `SparseRecord` values (partial updates) by reloading only the specified
    /// sub-fields, and falls back to full `load` for complete record values.
    pub(super) fn expand_embedded_reload_body(&self) -> TokenStream {
        let toasty = &self.toasty;

        let reload_arms = self.model.fields.iter().enumerate().map(|(index, field)| {
            let field_ident = &field.name.ident;
            let i = util::int(index);
            let field_name_str = field.name.ident.to_string();

            match &field.ty {
                FieldTy::Primitive(_ty) if field.attrs.serialize.is_some() => {
                    let serialize_attr = field.attrs.serialize.as_ref().unwrap();

                    let json_deserialize = quote! {
                        let json_str = <String as #toasty::Field>::load(value)?;
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
                            self.#field_ident = #assign;
                        }
                    }
                }
                FieldTy::Primitive(ty) => {
                    quote!(#i => <#ty as #toasty::Field>::reload(&mut self.#field_ident, value)?,)
                }
                _ => {
                    quote!(#i => self.#field_ident.unload(),)
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
                    *self = Self::load(value)?;
                    Ok(())
                }
            }
        }
    }
}
