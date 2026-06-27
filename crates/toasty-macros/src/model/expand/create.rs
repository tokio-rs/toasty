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

            impl #toasty::IntoExpr<#model_ident> for #create_struct_ident {
                fn into_expr(self) -> #toasty::stmt::Expr<#model_ident> {
                    self.stmt.into()
                }

                fn by_ref(&self) -> #toasty::stmt::Expr<#model_ident> {
                    todo!()
                }
            }

            impl #toasty::IntoExpr<Option<#model_ident>> for #create_struct_ident {
                fn into_expr(self) -> #toasty::stmt::Expr<Option<#model_ident>> {
                    self.stmt.into()
                }

                fn by_ref(&self) -> #toasty::stmt::Expr<Option<#model_ident>> {
                    todo!()
                }
            }

            impl #toasty::Assign<#model_ident> for #create_struct_ident {
                fn into_assignment(self) -> #toasty::stmt::Assignment<#model_ident> {
                    #toasty::stmt::set(
                        <Self as #toasty::IntoExpr<#model_ident>>::into_expr(self)
                    )
                }
            }

            impl #toasty::Assign<Option<#model_ident>> for #create_struct_ident {
                fn into_assignment(self) -> #toasty::stmt::Assignment<Option<#model_ident>> {
                    #toasty::stmt::set(
                        <Self as #toasty::IntoExpr<Option<#model_ident>>>::into_expr(self)
                    )
                }
            }

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

        // Item-collection children inherit the partition column from the
        // parent handle (R2.4) and have their sort key composed by
        // `AutoStrategy::ItemCollectionChildSortKey` (R7.1). Letting the
        // create-builder expose setters for either field would let the caller
        // override values the engine owns; today that produces a confusing
        // runtime `assert_eq!` panic on sk-encoder mismatch (B4.11 review).
        // Suppress both setters at macro-expansion when the model declares
        // `#[item_parent]`. The `#[item_parent]` field itself is already
        // suppressed via the `FieldTy::ItemParent` arm below (since B4.7).
        let suppressed_pk_indices: &[usize] = if self.model.item_parent_field.is_some() {
            &self.model.kind.as_root_unwrap().primary_key.fields
        } else {
            &[]
        };

        self.model
            .fields
            .iter()
            .enumerate()
            .map(move |(index, field)| {
                if suppressed_pk_indices.contains(&index) {
                    return TokenStream::new();
                }

                let name = &field.name.ident;
                let index_tokenized = util::int(index);

                match &field.ty {
                    FieldTy::BelongsTo(rel) => {
                        let ty = &rel.ty;

                        quote! {
                            #vis fn #name(mut self, #name: impl #toasty::IntoExpr<<#ty as #toasty::RelationOneField>::Expr>) -> Self {
                                // Silences unused field warning when the field is set on creation.
                                if false {
                                    let m = <#model_ident as #toasty::Load>::load(Default::default()).unwrap();
                                    let _ = &m.#name;
                                }

                                self.stmt.set(#index_tokenized, #name.into_expr());
                                self
                            }
                        }
                    }
                    // The create-builder setter for an ItemParent field would
                    // distribute the parent's PK into the child's PK fields,
                    // but item-collection children inherit those fields by
                    // schema convention (R2.4) — the create surface is
                    // resliced in B4.8/B7. For B4.7 we omit the setter.
                    FieldTy::ItemParent(_) => TokenStream::new(),
                    FieldTy::HasMany(rel) => {
                        if rel.via.is_some() {
                            TokenStream::new()
                        } else {
                            let plural = name;
                            let ty = &rel.ty;
                            let target = quote!(<#ty as #toasty::RelationManyField>::Target);

                            quote! {
                                #vis fn #plural(mut self, #plural: impl #toasty::IntoExpr<#toasty::List<#target>>) -> Self {
                                    self.stmt.insert_all(#index_tokenized, #plural.into_expr());
                                    self
                                }
                            }
                        }
                    }
                    FieldTy::HasOne(rel) => {
                        if rel.via.is_some() {
                            TokenStream::new()
                        } else {
                            let ty = &rel.ty;

                            quote! {
                                #vis fn #name(mut self, #name: impl #toasty::IntoExpr<<#ty as #toasty::RelationOneField>::Expr>) -> Self {
                                    self.stmt.set(#index_tokenized, #name.into_expr());
                                    self
                                }
                            }
                        }
                    }
                    FieldTy::Primitive(ty) => {
                        // The setter binds through the field's
                        // `Field::ExprTarget` — `Self` for scalars/`Vec<u8>`,
                        // `List<T>` for `Vec<T: Scalar>`. Trait dispatch
                        // routes each case correctly; no type parsing here.
                        quote! {
                            #vis fn #name(mut self, #name: impl IntoExpr<FieldExprTarget<#ty>>) -> Self {
                                self.stmt.set(#index_tokenized, #name.into_expr());
                                self
                            }
                        }
                    }
                }
            })
            .collect()
    }
}
