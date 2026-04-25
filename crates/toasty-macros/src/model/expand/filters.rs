use super::Expand;
use crate::model::schema::{FieldTy, Index, Model};

use hashbrown::HashMap;
use proc_macro2::{Span, TokenStream};
use quote::quote;

/// Combination of fields for which filter a method should be generated.
#[derive(Debug)]
pub(super) struct Filter {
    /// Fields to filter by
    fields: Vec<usize>,

    /// When true, only include the filter on relation structs
    only_relation: bool,

    /// Get method identifier
    get_method_ident: syn::Ident,

    /// Filter method identifer
    pub(super) filter_method_ident: syn::Ident,

    /// Update method identifier
    update_method_ident: syn::Ident,

    /// Delete method identifier
    delete_method_ident: syn::Ident,
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

                quote!(
                    #get_method
                    #filter_method
                )
            })
            .collect()
    }

    fn expand_model_get_method(&self, filter: &Filter, self_into_query: bool) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let get_method_ident = &filter.get_method_ident;
        let update_method_ident = &filter.update_method_ident;
        let delete_method_ident = &filter.delete_method_ident;
        let filter_method_ident = &filter.filter_method_ident;
        let args: Vec<_> = self.expand_filter_args(filter).collect();
        let arg_idents: Vec<_> = self.expand_filter_arg_idents(filter).collect();
        let update_query_struct_ident = &self.model.kind.as_root_unwrap().update_struct_ident;

        let self_arg;
        let base;

        if self_into_query {
            self_arg = quote!(self,);
            base = quote!(self.);
        } else {
            self_arg = quote!();
            base = quote!(Self::);
        }

        quote! {
            #vis async fn #get_method_ident(#self_arg executor: &mut dyn #toasty::Executor, #( #args ),* ) -> #toasty::Result<#model_ident> {
                #base #filter_method_ident( #( #arg_idents ),* )
                    .get(executor)
                    .await
            }

            #vis fn #update_method_ident(#self_arg #( #args ),* ) -> #update_query_struct_ident {
                #base #filter_method_ident( #( #arg_idents ),* ).update()
            }

            #vis async fn #delete_method_ident(#self_arg executor: &mut dyn #toasty::Executor, #( #args ),* ) -> #toasty::Result<()> {
                #base #filter_method_ident( #( #arg_idents ),* )
                    .delete()
                    .exec(executor)
                    .await
            }
        }
    }

    fn expand_model_filter_method(&self, filter: &Filter, self_into_query: bool) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let query_struct_ident = &self.model.kind.as_root_unwrap().query_struct_ident;
        let filter_method_ident = &filter.filter_method_ident;
        let args = self.expand_filter_args(filter);
        let arg_idents = self.expand_filter_arg_idents(filter);
        let self_arg;
        let body;

        if self_into_query {
            let expr = self.expand_query_filter_expr(filter);

            self_arg = quote!(self,);
            body = quote! {
                #query_struct_ident::from_stmt({ use #toasty::IntoStatement; self.into_statement().into_query().unwrap() }).filter( #expr )
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

    pub(super) fn expand_query_filter_methods(&self) -> TokenStream {
        self.filters
            .iter()
            // .filter(|f| !f.only_relation)
            .map(|filter| {
                let get_method = self.expand_model_get_method(filter, true);
                let filter_method = self.expand_query_filter_method(filter);

                quote! {
                    #get_method
                    #filter_method
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

                quote!(
                    #get_method
                    #filter_method
                )
            })
            .collect()
    }

    fn expand_query_filter_method(&self, filter: &Filter) -> TokenStream {
        let vis = &self.model.vis;
        let query_struct_ident = &self.model.kind.as_root_unwrap().query_struct_ident;
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

            let Some(_index) = self.find_index(&fields) else {
                continue;
            };

            if let Some(filter) = self.filters.get_mut(&fields) {
                filter.only_relation = false;
            } else {
                self.filters.insert(
                    fields.clone(),
                    Filter {
                        fields: fields.clone(),
                        only_relation: false,
                        get_method_ident: self.method_ident(&fields, "get"),
                        filter_method_ident: self.method_ident(&fields, "filter"),
                        update_method_ident: self.method_ident(&fields, "update"),
                        delete_method_ident: self.method_ident(&fields, "delete"),
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
                            only_relation: true,
                            get_method_ident: self.method_ident(&fields, "get"),
                            filter_method_ident: self.method_ident(&fields, "filter"),
                            update_method_ident: self.method_ident(&fields, "update"),
                            delete_method_ident: self.method_ident(&fields, "delete"),
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

    fn method_ident(&self, fields: &[usize], prefix: &str) -> syn::Ident {
        let mut name = prefix.to_string();

        let mut prefix = "_by_";

        for index in fields {
            name.push_str(prefix);
            name.push_str(&self.model.fields[*index].name.as_str());

            prefix = "_and_";
        }

        syn::Ident::new(&name, Span::call_site())
    }
}
