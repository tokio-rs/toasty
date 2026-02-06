use super::{util, Expand};
use crate::schema::{FieldTy, Index, Model};

use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::collections::HashMap;

/// Combination of fields for which filter a method should be generated.
#[derive(Debug)]
pub(super) struct Filter {
    /// Fields to filter by
    fields: Vec<usize>,

    /// When true, include a batch filter method
    batch: bool,

    /// When true, only include the filter on relation structs
    only_relation: bool,

    /// Get method identifier
    get_method_ident: syn::Ident,

    /// Filter method identifer
    pub(super) filter_method_ident: syn::Ident,

    /// Filter method batch identifier
    filter_method_batch_ident: syn::Ident,
}

struct BuildModelFilters<'a> {
    model: &'a Model,
    filters: HashMap<Vec<usize>, Filter>,
}

impl Expand<'_> {
    pub(super) fn expand_model_filter_methods(&self) -> TokenStream {
        self.filters
            .iter()
            .filter(|f| !f.only_relation)
            .map(|filter| {
                let get_method = self.expand_model_get_method(filter, false);
                let filter_method = self.expand_model_filter_method(filter, false);

                let filter_batch_method = if filter.batch {
                    Some(self.expand_model_filter_batch_method(filter, false))
                } else {
                    None
                };

                quote!(
                    #get_method
                    #filter_method
                    #filter_batch_method
                )
            })
            .collect()
    }

    fn expand_model_get_method(&self, filter: &Filter, self_into_select: bool) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let get_method_ident = &filter.get_method_ident;
        let filter_method_ident = &filter.filter_method_ident;
        let args = self.expand_filter_args(filter);
        let arg_idents = self.expand_filter_arg_idents(filter);
        let self_arg;
        let base;

        if self_into_select {
            self_arg = quote!(self,);
            base = quote!(self.);
        } else {
            self_arg = quote!();
            base = quote!(Self::);
        }

        quote! {
            #vis async fn #get_method_ident(#self_arg db: &#toasty::Db, #( #args ),* ) -> #toasty::Result<#model_ident> {
                #base #filter_method_ident( #( #arg_idents ),* )
                    .get(db)
                    .await
            }
        }
    }

    fn expand_model_filter_method(&self, filter: &Filter, self_into_select: bool) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let query_struct_ident = &self.model.kind.expect_root().query_struct_ident;
        let filter_method_ident = &filter.filter_method_ident;
        let args = self.expand_filter_args(filter);
        let arg_idents = self.expand_filter_arg_idents(filter);
        let self_arg;
        let body;

        if self_into_select {
            let expr = self.expand_query_filter_expr(filter);

            self_arg = quote!(self,);
            body = quote! {
                use #toasty::IntoSelect;
                #query_struct_ident::from_stmt(self.into_select()).filter( #expr )
            };
        } else {
            self_arg = quote!();
            body = quote! {
                #query_struct_ident::default()
                    .#filter_method_ident( #( #arg_idents ),* )
            };
        }

        quote! {
            #vis fn #filter_method_ident( #self_arg #( #args ),* ) -> #query_struct_ident {
                #body
            }
        }
    }

    fn expand_model_filter_batch_method(
        &self,
        filter: &Filter,
        self_into_select: bool,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let query_struct_ident = &self.model.kind.expect_root().query_struct_ident;
        let filter_method_batch_ident = &filter.filter_method_batch_ident;
        let bound = self.expand_filter_batch_arg_bound(filter);
        let self_arg;
        let query;

        if self_into_select {
            self_arg = quote!(self,);
            query = quote!(#query_struct_ident::from_stmt(self.into_select()));
        } else {
            self_arg = quote!();
            query = quote!(#query_struct_ident::default());
        }

        quote! {
            #vis fn #filter_method_batch_ident(#self_arg keys: impl #toasty::IntoExpr<[#bound]>) -> #query_struct_ident {
                use #toasty::IntoSelect;
                #query.#filter_method_batch_ident( keys )
            }
        }
    }

    pub(super) fn expand_query_filter_methods(&self) -> TokenStream {
        self.filters
            .iter()
            // .filter(|f| !f.only_relation)
            .map(|filter| {
                let get_method = self.expand_model_get_method(filter, true);
                let filter_method = self.expand_query_filter_method(filter);
                let filter_batch_method = if filter.batch {
                    Some(self.expand_query_filter_batch_method(filter))
                } else {
                    None
                };

                quote! {
                    #get_method
                    #filter_method
                    #filter_batch_method
                }
            })
            .collect()
    }

    pub(super) fn expand_relation_filter_methods(&self) -> TokenStream {
        self.filters
            .iter()
            .map(|filter| {
                let get_method = self.expand_model_get_method(filter, true);
                let filter_method = self.expand_model_filter_method(filter, true);

                let filter_batch_method = if filter.batch {
                    Some(self.expand_model_filter_batch_method(filter, true))
                } else {
                    None
                };

                quote!(
                    #get_method
                    #filter_method
                    #filter_batch_method
                )
            })
            .collect()
    }

    fn expand_query_filter_method(&self, filter: &Filter) -> TokenStream {
        let vis = &self.model.vis;
        let query_struct_ident = &self.model.kind.expect_root().query_struct_ident;
        let filter_method_ident = &filter.filter_method_ident;
        let args = self.expand_filter_args(filter);
        let expr = self.expand_query_filter_expr(filter);

        quote! {
            #vis fn #filter_method_ident(self, #( #args ),* ) -> #query_struct_ident {
                self.filter(#expr)
            }
        }
    }

    fn expand_query_filter_expr(&self, filter: &Filter) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        let body = filter.fields.iter().map(|index| {
            let field = &self.model.fields[*index];
            let field_ident = &field.name.ident;

            quote!(#model_ident::fields().#field_ident().eq(#field_ident))
        });

        if filter.fields.len() == 1 {
            quote!(#( #body )*)
        } else {
            quote!(#toasty::stmt::Expr::and_all( [ #( #body ),* ] ))
        }
    }

    fn expand_query_filter_batch_method(&self, filter: &Filter) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_struct_ident = &self.model.kind.expect_root().query_struct_ident;
        let query_filter_batch_ident = &filter.filter_method_batch_ident;
        let bound = self.expand_filter_batch_arg_bound(filter);

        let lhs = filter.fields.iter().map(|index| {
            let field = &self.model.fields[*index];
            let field_ident = &field.name.ident;
            quote!(#model_ident::fields().#field_ident())
        });

        let lhs = if filter.fields.len() == 1 {
            quote!(#( #lhs )*)
        } else {
            quote!( ( #( #lhs ),* ) )
        };

        quote! {
            #vis fn #query_filter_batch_ident(self, keys: impl #toasty::IntoExpr<[#bound]> ) -> #query_struct_ident {
                self.filter( #toasty::stmt::Expr::in_list( #lhs, keys ) )
            }
        }
    }

    pub(crate) fn expand_model_into_expr_body(&self, by_ref: bool) -> TokenStream {
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

    pub(crate) fn expand_embedded_into_expr_body(&self, by_ref: bool) -> TokenStream {
        let toasty = &self.toasty;

        // For embedded types, create a record expression from all fields
        // Currently only primitive fields are supported in embedded types
        let field_exprs = self.model.fields.iter().map(|field| {
            let field_ident = &field.name.ident;
            let ty = match &field.ty {
                FieldTy::Primitive(ty) => ty,
                _ => {
                    // Relations and nested embedded types are not yet supported
                    panic!("only primitive fields are supported in embedded types")
                }
            };

            let into_expr = if by_ref {
                quote!((&self.#field_ident))
            } else {
                quote!(self.#field_ident)
            };

            quote! {
                {
                    let expr: #toasty::stmt::Expr<#ty> = #toasty::IntoExpr::into_expr(#into_expr);
                    let untyped: #toasty::core_stmt::Expr = expr.into();
                    untyped
                }
            }
        });

        quote! {
            #toasty::stmt::Expr::from_untyped(
                #toasty::core_stmt::Expr::record([
                    #( #field_exprs ),*
                ])
            )
        }
    }

    /// Generates the body for loading a model or embedded type from a Value.
    ///
    /// This method is used by both:
    /// - Root models (in `Model::load`) - supports all field types
    /// - Embedded types (in `Primitive::load`) - only primitive fields
    ///
    /// The generated code pattern matches on `Value::Record`, extracts fields,
    /// and constructs the struct.
    pub(crate) fn expand_load_body(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;

        // Generate field loading expressions
        let field_loads = self.model.fields.iter().enumerate().map(|(index, field)| {
            let field_ident = &field.name.ident;
            let index_tokenized = util::int(index);

            match &field.ty {
                FieldTy::Primitive(ty) => {
                    quote!(#field_ident: <#ty as #toasty::stmt::Primitive>::load(record[#index_tokenized].take())?,)
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
                #toasty::Value::Record(mut record) => {
                    Ok(#model_ident {
                        #( #field_loads )*
                    })
                }
                value => Err(#toasty::Error::type_conversion(value, stringify!(#model_ident))),
            }
        }
    }

    fn expand_filter_args<'b>(
        &'b self,
        filter: &'b Filter,
    ) -> impl Iterator<Item = TokenStream> + 'b {
        let toasty = &self.toasty;

        filter.fields.iter().map(move |index| {
            let field = &self.model.fields[*index];
            let name = &field.name.ident;
            let ty = match &field.ty {
                FieldTy::Primitive(ty) => ty,
                _ => todo!(),
            };

            quote!(#name: impl #toasty::IntoExpr<#ty>)
        })
    }

    fn expand_filter_batch_arg_bound(&self, filter: &Filter) -> TokenStream {
        let parts = filter.fields.iter().map(move |index| {
            let field = &self.model.fields[*index];
            let ty = match &field.ty {
                FieldTy::Primitive(ty) => ty,
                _ => todo!(),
            };

            quote!(#ty)
        });

        if filter.fields.len() == 1 {
            quote!( #( #parts )* )
        } else {
            quote!( ( #( #parts ),* ) )
        }
    }

    pub(super) fn expand_filter_arg_idents<'b>(
        &'b self,
        filter: &'b Filter,
    ) -> impl Iterator<Item = TokenStream> + 'b {
        filter.fields.iter().map(move |field| {
            let name = &self.model.fields[*field].name.ident;
            quote!(#name)
        })
    }

    pub(super) fn primary_key_filter(&self) -> &Filter {
        let fields = self
            .model
            .primary_key_fields()
            .expect("primary_key_filter called on model without primary key")
            .map(|field| field.id)
            .collect::<Vec<_>>();

        self.filters.iter().find(|f| f.fields == fields).unwrap()
    }
}

impl Filter {
    pub(super) fn build_model_filters(model: &Model) -> Vec<Self> {
        BuildModelFilters {
            model,
            filters: HashMap::new(),
        }
        .build()
    }
}

impl<'a> BuildModelFilters<'a> {
    fn build(mut self) -> Vec<Filter> {
        self.recurse(&[]);
        self.filters.into_values().collect()
    }

    fn recurse(&mut self, prefix: &[usize]) {
        for field in &self.model.fields {
            let FieldTy::Primitive(_primitive) = &field.ty else {
                continue;
            };

            let fields = prefix
                .iter()
                .chain(Some(&field.id))
                .copied()
                .collect::<Vec<_>>();

            let Some(index) = self.find_index(&fields) else {
                continue;
            };

            if let Some(filter) = self.filters.get_mut(&fields) {
                filter.batch |= index.primary_key && index.fields.len() == fields.len();
                filter.only_relation = false;
            } else {
                self.filters.insert(
                    fields.clone(),
                    Filter {
                        fields: fields.clone(),
                        batch: index.primary_key && index.fields.len() == fields.len(),
                        only_relation: false,
                        get_method_ident: self.method_ident(&fields, "get", None),
                        filter_method_ident: self.method_ident(&fields, "filter", None),
                        filter_method_batch_ident: self.method_ident(
                            &fields,
                            "filter",
                            Some("batch"),
                        ),
                    },
                );
            }

            // Now add fitlers for relation structs

            for i in 0..fields.len() {
                let fields = fields[i..].to_vec();

                if !self.filters.contains_key(&fields) {
                    self.filters.insert(
                        fields.clone(),
                        Filter {
                            fields: fields.clone(),
                            batch: false,
                            only_relation: true,
                            get_method_ident: self.method_ident(&fields, "get", None),
                            filter_method_ident: self.method_ident(&fields, "filter", None),
                            filter_method_batch_ident: self.method_ident(
                                &fields,
                                "filter",
                                Some("batch"),
                            ),
                        },
                    );
                }
            }

            self.recurse(&fields);
        }
    }

    fn find_index(&self, fields: &[usize]) -> Option<&'a Index> {
        for index in &self.model.indices {
            if index.fields.len() < fields.len() {
                continue;
            }

            if fields
                .iter()
                .zip(index.fields.iter())
                .all(|(field_id, index_field)| *field_id == index_field.field)
            {
                return Some(index);
            }
        }

        None
    }

    fn method_ident(&self, fields: &[usize], prefix: &str, suffix: Option<&str>) -> syn::Ident {
        let mut name = prefix.to_string();

        let mut prefix = "_by_";

        for index in fields {
            name.push_str(prefix);
            name.push_str(&self.model.fields[*index].name.ident.to_string());

            prefix = "_and_";
        }

        if let Some(suffix) = suffix {
            name.push('_');
            name.push_str(suffix);
        }

        syn::Ident::new(&name, Span::call_site())
    }
}
