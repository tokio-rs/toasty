use super::*;

use std::collections::HashMap;

/// Combination of fields for which filter a method should be generated.
pub(super) struct Filter {
    /// Fields to filter by
    fields: Vec<app::FieldId>,

    /// When true, include a batch filter method
    batch: bool,

    /// When true, only include the filter on relation structs
    only_relation: bool,
}

struct BuildModelFilters<'a> {
    model: &'a app::Model,
    filters: HashMap<Vec<app::FieldId>, Filter>,
}

impl Filter {
    pub(super) fn build_model_filters(model: &app::Model) -> Vec<Filter> {
        BuildModelFilters {
            model,
            filters: HashMap::new(),
        }
        .build()
    }
}

impl<'a> Generator<'a> {
    pub(super) fn gen_model_filter_methods(&self, depth: usize) -> TokenStream {
        self.filters
            .iter()
            .filter(|f| !f.only_relation)
            .map(|filter| {
                let get_method = self.gen_model_get_method(filter, depth, false);
                let filter_method = self.gen_model_filter_method(filter, depth, false);

                let filter_batch_method = if filter.batch {
                    Some(self.gen_model_filter_batch_method(filter, depth, false))
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

    fn gen_model_get_method(
        &self,
        filter: &Filter,
        depth: usize,
        self_into_select: bool,
    ) -> TokenStream {
        let struct_name = self.self_struct_name();
        let ident = self.get_method_ident(&filter.fields);
        let filter_ident = self.filter_method_ident(&filter.fields);
        let args = self.gen_filter_args(filter, depth);
        let arg_idents = self.gen_filter_arg_idents(&filter.fields);
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
            pub async fn #ident(#self_arg db: &Db, #( #args ),* ) -> Result<#struct_name> {
                #base #filter_ident( #( #arg_idents ),* )
                    .get(db)
                    .await
            }
        }
    }

    fn gen_model_filter_method(
        &self,
        filter: &Filter,
        depth: usize,
        self_into_select: bool,
    ) -> TokenStream {
        let ident = self.filter_method_ident(&filter.fields);
        let args = self.gen_filter_args(filter, depth);
        let arg_idents = self.gen_filter_arg_idents(&filter.fields);
        let self_arg;
        let body;

        if self_into_select {
            let expr = self.gen_query_filter_expr(filter);

            self_arg = quote!(self,);
            body = quote! {
                Query::from_stmt(self.into_select()).filter( #expr )
            };
        } else {
            self_arg = quote!();
            body = quote! {
                Query::default().#ident( #( #arg_idents ),* )
            };
        }

        quote! {
            pub fn #ident( #self_arg #( #args ),* ) -> Query {
                #body
            }
        }
    }

    fn gen_model_filter_batch_method(
        &self,
        filter: &Filter,
        depth: usize,
        self_into_select: bool,
    ) -> TokenStream {
        let ident = self.filter_method_batch_ident(&filter.fields);
        let bound = self.gen_filter_batch_arg_bound(filter, depth);
        let self_arg;
        let query;

        if self_into_select {
            self_arg = quote!(self,);
            query = quote!(Query::from_stmt(self.into_select()));
        } else {
            self_arg = quote!();
            query = quote!(Query::default());
        }

        quote! {
            pub fn #ident(#self_arg keys: impl IntoExpr<[#bound]>) -> Query {
                #query.#ident( keys )
            }
        }
    }

    pub(super) fn gen_query_filter_methods(&self) -> TokenStream {
        self.filters
            .iter()
            .filter(|f| !f.only_relation)
            .map(|filter| {
                let get_method = self.gen_model_get_method(filter, 0, true);
                let filter_method = self.gen_query_filter_method(filter);
                let filter_batch_method = if filter.batch {
                    Some(self.gen_query_filter_batch_method(filter))
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

    pub(super) fn gen_relation_filter_methods(&self) -> TokenStream {
        self.filters
            .iter()
            .map(|filter| {
                let get_method = self.gen_model_get_method(filter, 1, true);
                let filter_method = self.gen_model_filter_method(filter, 1, true);

                let filter_batch_method = if filter.batch {
                    Some(self.gen_model_filter_batch_method(filter, 1, true))
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

    fn gen_query_filter_method(&self, filter: &Filter) -> TokenStream {
        let ident = self.filter_method_ident(&filter.fields);
        let args = self.gen_filter_args(filter, 0);
        let expr = self.gen_query_filter_expr(filter);

        quote! {
            pub fn #ident(self, #( #args ),* ) -> Query {
                self.filter(#expr)
            }
        }
    }

    fn gen_query_filter_expr(&self, filter: &Filter) -> TokenStream {
        let struct_name = self.self_struct_name();
        let body = filter.fields.iter().map(|field| {
            let name = self.field_name(*field);
            let path = self.field_const_name(field);

            quote!(#struct_name::#path.eq(#name))
        });

        if filter.fields.len() == 1 {
            quote!(#( #body )*)
        } else {
            quote!(stmt::Expr::and_all( [ #( #body ),* ] ))
        }
    }

    fn gen_query_filter_batch_method(&self, filter: &Filter) -> TokenStream {
        let struct_name = self.self_struct_name();
        let ident = self.filter_method_batch_ident(&filter.fields);
        let bound = self.gen_filter_batch_arg_bound(filter, 0);

        let lhs = filter.fields.iter().map(|field| {
            let path = self.field_const_name(field);
            quote!(#struct_name::#path)
        });

        let lhs = if filter.fields.len() == 1 {
            quote!(#( #lhs )*)
        } else {
            quote!( ( #( #lhs ),* ) )
        };

        quote! {
            pub fn #ident(self, keys: impl IntoExpr<[#bound]> ) -> Query {
                self.filter( stmt::Expr::in_list( #lhs, keys ) )
            }
        }
    }

    pub(crate) fn gen_model_into_expr_body(&self, by_ref: bool) -> TokenStream {
        if self.model.primary_key.fields.len() == 1 {
            let expr = self.model.primary_key_fields().map(|field| {
                let name = self.field_name(field);
                let ty = self.field_ty(field, 0);

                let into_expr = if by_ref {
                    quote!((&self.#name))
                } else {
                    quote!(self.#name)
                };

                quote! {
                    let expr: stmt::Expr<#ty> = #into_expr.into_expr();
                    expr.cast()
                }
            });

            quote!( #( #expr )* )
        } else {
            let expr = self.model.primary_key_fields().map(|field| {
                let name = self.field_name(field);
                let amp = if by_ref { quote!(&) } else { quote!() };
                quote!( #amp self.#name)
            });

            let ty = self
                .model
                .primary_key_fields()
                .map(|field| self.field_ty(field, 0));

            quote! {
                let expr: stmt::Expr<( #( #ty ),* )> = ( #( #expr ),* ).into_expr();
                expr.cast()
            }
        }
    }

    pub(super) fn gen_model_into_select_body(&self, by_ref: bool) -> TokenStream {
        let fields = self
            .model
            .primary_key_fields()
            .map(|field| field.id)
            .collect::<Vec<_>>();

        let ident = self.filter_method_ident(&fields);
        let arg_idents = self.gen_filter_arg_idents(&fields);

        let amp = if by_ref { quote!(&) } else { quote!() };

        quote! {
            Query::default()
                .#ident( #( #amp self.#arg_idents ),* )
                .stmt
        }
    }

    fn get_method_ident(&self, fields: &[app::FieldId]) -> syn::Ident {
        self.method_ident(fields, "get_by", None)
    }

    fn filter_method_ident(&self, fields: &[app::FieldId]) -> syn::Ident {
        self.method_ident(fields, "filter_by", None)
    }

    fn filter_method_batch_ident(&self, fields: &[app::FieldId]) -> syn::Ident {
        self.method_ident(fields, "filter_by", Some("batch"))
    }

    fn method_ident(
        &self,
        fields: &[app::FieldId],
        prefix: &str,
        suffix: Option<&str>,
    ) -> syn::Ident {
        let mut name = prefix.to_string();

        let mut prefix = "_";

        for field in fields {
            name.push_str(prefix);
            name.push_str(&self.model.fields[field.index].name);

            prefix = "_and_";
        }

        if let Some(suffix) = suffix {
            name.push_str("_");
            name.push_str(suffix);
        }

        util::ident(&name)
    }

    fn gen_filter_args<'b>(
        &'b self,
        filter: &'b Filter,
        depth: usize,
    ) -> impl Iterator<Item = TokenStream> + 'b {
        filter.fields.iter().map(move |field| {
            let name = self.field_name(*field);
            let ty = self.field_ty(*field, depth);

            quote!(#name: impl IntoExpr<#ty>    )
        })
    }

    fn gen_filter_batch_arg_bound(&self, filter: &Filter, depth: usize) -> TokenStream {
        let parts = filter.fields.iter().map(move |field| {
            let ty = self.field_ty(*field, depth);
            quote!(#ty)
        });

        if filter.fields.len() == 1 {
            quote!( #( #parts )* )
        } else {
            quote!( ( #( #parts ),* ) )
        }
    }

    fn gen_filter_arg_idents<'b>(
        &'b self,
        fields: &'b [app::FieldId],
    ) -> impl Iterator<Item = TokenStream> + 'b {
        fields.iter().map(move |field| {
            let name = self.field_name(*field);

            quote!(#name)
        })
    }
}

impl<'a> BuildModelFilters<'a> {
    fn build(mut self) -> Vec<Filter> {
        self.recurse(&[]);
        self.filters.into_iter().map(|(_, filter)| filter).collect()
    }

    fn recurse(&mut self, prefix: &[app::FieldId]) {
        for field in &self.model.fields {
            let app::FieldTy::Primitive(_primitive) = &field.ty else {
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
                        },
                    );
                }
            }

            self.recurse(&fields);
        }
    }

    fn find_index(&self, fields: &[app::FieldId]) -> Option<&'a app::ModelIndex> {
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
}
