use super::{Expand, util};
use crate::model::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

impl Expand<'_> {
    pub(super) fn expand_create_builder(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let model_span = model_ident.span();
        let create_struct_ident = &self.model.kind.as_root_unwrap().create_struct_ident;
        let create_methods = self.expand_create_methods();
        let default_stmts = self.expand_create_default_stmts();
        let conversions = self.expand_create_conversions(
            create_struct_ident,
            quote!(self.stmt.into()),
            quote!(todo!()),
        );

        // Span the struct definition to the model ident so that "method not
        // found for this struct" errors point at `struct User`, not the derive
        // attribute.
        let struct_def = quote_spanned! { model_span=>
            #[derive(Clone)]
            #vis struct #create_struct_ident {
                stmt: #toasty::stmt::Insert<#model_ident>,
            }
        };

        quote! {
            #struct_def

            impl #create_struct_ident {
                #create_methods

                #vis async fn exec(self, executor: &mut dyn #toasty::Executor) -> #toasty::Result<#model_ident> {
                    executor.exec(self.stmt.into()).await
                }
            }

            impl #toasty::IntoInsert for #create_struct_ident {
                type Model = #model_ident;

                fn into_insert(self) -> #toasty::stmt::Insert<#model_ident> {
                    self.stmt
                }
            }

            impl #toasty::IntoStatement for #create_struct_ident {
                type Returning = #model_ident;

                fn into_statement(self) -> #toasty::Statement<#model_ident> {
                    self.stmt.into()
                }
            }

            #conversions

            impl Default for #create_struct_ident {
                fn default() -> #create_struct_ident {
                    let mut s = #create_struct_ident {
                        stmt: #toasty::stmt::Insert::blank_single(),
                    };
                    #default_stmts
                    s
                }
            }
        }
    }

    /// Generates expression and assignment conversions for a construction
    /// builder, accepting both the model and its nullable form.
    pub(super) fn expand_create_conversions(
        &self,
        builder_ident: &syn::Ident,
        into_expr: TokenStream,
        by_ref: TokenStream,
    ) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;

        [
            (quote!(#model_ident), into_expr, by_ref),
            (
                quote!(#toasty::Option<#model_ident>),
                quote!(<Self as #toasty::IntoExpr<#model_ident>>::into_expr(self).some()),
                quote!(<Self as #toasty::IntoExpr<#model_ident>>::by_ref(self).some()),
            ),
        ]
        .into_iter()
        .map(|(target, into_expr, by_ref)| {
            quote! {
                impl #toasty::IntoExpr<#target> for #builder_ident {
                    fn into_expr(self) -> #toasty::stmt::Expr<#target> {
                        #into_expr
                    }

                    fn by_ref(&self) -> #toasty::stmt::Expr<#target> {
                        #by_ref
                    }
                }

                impl #toasty::Assign<#target> for #builder_ident {
                    fn into_assignment(self) -> #toasty::stmt::Assignment<#target> {
                        #toasty::stmt::set(
                            <Self as #toasty::IntoExpr<#target>>::into_expr(self)
                        )
                    }
                }
            }
        })
        .collect()
    }

    fn expand_create_default_stmts(&self) -> TokenStream {
        let toasty = &self.toasty;

        self.model
            .fields
            .iter()
            .enumerate()
            .filter_map(|(index, field)| {
                // #[default] takes priority over #[update] on create
                let expr = field
                    .attrs
                    .default_expr
                    .as_ref()
                    .or(field.attrs.update_expr.as_ref())?;
                let FieldTy::Primitive(ty) = &field.ty else {
                    return None;
                };
                let index_tokenized = util::int(index);
                Some(quote! {
                    s.stmt.set(
                        #index_tokenized,
                        <#ty as #toasty::IntoExpr<<#ty as #toasty::Field>::ExprTarget>>::into_expr(#expr),
                    );
                })
            })
            .collect()
    }

    fn expand_create_methods(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;

        self.model
            .fields
            .iter()
            .enumerate()
            .filter_map(|(index, field)| {
                let target = self.expand_setter_target(&field.ty)?;
                let into_expr = if field.ty.is_primitive() {
                    quote!(IntoExpr)
                } else {
                    quote!(#toasty::IntoExpr)
                };
                let name = &field.name.ident;
                let index = util::int(index);
                let method = if matches!(field.ty, FieldTy::HasMany(_)) {
                    quote!(insert_all)
                } else {
                    quote!(set)
                };
                let field_use = matches!(field.ty, FieldTy::BelongsTo(_)).then(|| quote! {
                    // Silences unused field warning when the field is set on creation.
                    if false {
                        let m = <#model_ident as #toasty::Load>::load(Default::default()).unwrap();
                        let _ = &m.#name;
                    }
                });

                Some(quote! {
                    #vis fn #name(mut self, #name: impl #into_expr<#target>) -> Self {
                        #field_use
                        self.stmt.#method(#index, #name.into_expr());
                        self
                    }
                })
            })
            .collect()
    }
}
