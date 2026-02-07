use super::Expand;
use crate::schema::ModelKind;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_impls(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;

        let (
            query_struct_ident,
            create_struct_ident,
            update_struct_ident,
            update_query_struct_ident,
        ) = match &self.model.kind {
            ModelKind::Root(root) => (
                &root.query_struct_ident,
                &root.create_struct_ident,
                &root.update_struct_ident,
                &root.update_query_struct_ident,
            ),
            ModelKind::Embedded(_) => {
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
        let into_select_body_ref = self.expand_model_into_select_body(true);
        let into_select_body_value = self.expand_model_into_select_body(false);
        let into_expr_body_ref = self.expand_model_into_expr_body(true);
        let into_expr_body_val = self.expand_model_into_expr_body(false);

        quote! {
            impl #model_ident {
                #model_fields
                #filter_methods
                #relation_methods

                #vis fn create() -> #create_struct_ident {
                    #create_struct_ident::default()
                }

                #vis fn create_many() -> #toasty::CreateMany<#model_ident> {
                    #toasty::CreateMany::default()
                }

                #vis fn update(&mut self) -> #update_struct_ident {
                    use #toasty::IntoSelect;
                    let query = #update_query_struct_ident::from(self.into_select());
                    #update_struct_ident {
                        model: self,
                        query,
                    }
                }

                #vis fn all() -> #query_struct_ident {
                    #query_struct_ident::default()
                }

                #vis fn filter(expr: #toasty::stmt::Expr<bool>) -> #query_struct_ident {
                    #query_struct_ident::from_stmt(#toasty::stmt::Select::filter(expr))
                }

                #vis async fn delete(self, db: &#toasty::Db) -> #toasty::Result<()> {
                    use #toasty::IntoSelect;
                    let stmt = self.into_select().delete();
                    db.exec(stmt).await?;
                    Ok(())
                }
            }

            impl #toasty::Register for #model_ident {
                fn id() -> #toasty::ModelId {
                    static ID: std::sync::OnceLock<#toasty::ModelId> = std::sync::OnceLock::new();
                    *ID.get_or_init(|| #toasty::generate_unique_id())
                }

                #model_schema
            }

            impl #toasty::Model for #model_ident {
                type Query = #query_struct_ident;
                type Create = #create_struct_ident;
                type Update<'a> = #update_struct_ident<'a>;
                type UpdateQuery = #update_query_struct_ident;

                fn load(value: #toasty::Value) -> #toasty::Result<Self> {
                    #load_body
                }
            }

            impl #toasty::Relation for #model_ident {
                type Model = #model_ident;
                type Expr = #model_ident;
                type Query = #query_struct_ident;
                type Many = Many;
                type ManyField = ManyField;
                type One = One;
                type OneField = OneField;
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

            impl #toasty::stmt::IntoExpr<[#model_ident]> for #model_ident {
                fn into_expr(self) -> #toasty::stmt::Expr<[#model_ident]> {
                    #toasty::stmt::Expr::list([self])
                }

                fn by_ref(&self) -> #toasty::stmt::Expr<[#model_ident]> {
                    #toasty::stmt::Expr::list([self])
                }
            }

            impl #toasty::stmt::IntoSelect for &#model_ident {
                type Model = #model_ident;

                fn into_select(self) -> #toasty::stmt::Select<Self::Model> {
                    #into_select_body_ref
                }
            }

            impl #toasty::stmt::IntoSelect for &mut #model_ident {
                type Model = #model_ident;

                fn into_select(self) -> #toasty::stmt::Select<Self::Model> {
                    (&*self).into_select()
                }
            }

            impl #toasty::stmt::IntoSelect for #model_ident {
                type Model = #model_ident;

                fn into_select(self) -> #toasty::stmt::Select<Self::Model> {
                    #into_select_body_value
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

    pub(super) fn expand_model_into_select_body(&self, by_ref: bool) -> TokenStream {
        let filter = self.primary_key_filter();
        let query_struct_ident = &self.model.kind.expect_root().query_struct_ident;
        let filter_method_ident = &filter.filter_method_ident;
        let arg_idents = self.expand_filter_arg_idents(filter);
        let amp = if by_ref { quote!(&) } else { quote!() };

        quote! {
            #query_struct_ident::default()
                .#filter_method_ident( #( #amp self.#arg_idents ),* )
                .stmt
        }
    }
}
