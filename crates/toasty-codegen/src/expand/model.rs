use super::{util, Expand};
use crate::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_impls(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let query_struct_ident = &self.model.query_struct_ident;
        let create_struct_ident = &self.model.create_struct_ident;
        let update_struct_ident = &self.model.update_struct_ident;
        let update_query_struct_ident = &self.model.update_query_struct_ident;
        let model_schema = self.expand_model_schema();
        let model_fields = self.expand_model_field_struct_init();
        let struct_load_fields = self.expand_struct_load_fields();
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

            impl #toasty::Model for #model_ident {
                type Query = #query_struct_ident;
                type Create = #create_struct_ident;
                type Update<'a> = #update_struct_ident<'a>;
                type UpdateQuery = #update_query_struct_ident;

                fn id() -> #toasty::ModelId {
                    static ID: std::sync::OnceLock<#toasty::ModelId> = std::sync::OnceLock::new();
                    *ID.get_or_init(|| #toasty::generate_unique_id())
                }

                fn load(mut record: #toasty::ValueRecord) -> #toasty::Result<Self> {
                    Ok(Self {
                        #struct_load_fields
                    })
                }

                #model_schema
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

    pub(super) fn expand_model_into_select_body(&self, by_ref: bool) -> TokenStream {
        let filter = self.primary_key_filter();
        let query_struct_ident = &self.model.query_struct_ident;
        let filter_method_ident = &filter.filter_method_ident;
        let arg_idents = self.expand_filter_arg_idents(filter);
        let amp = if by_ref { quote!(&) } else { quote!() };

        quote! {
            #query_struct_ident::default()
                .#filter_method_ident( #( #amp self.#arg_idents ),* )
                .stmt
        }
    }

    fn expand_struct_load_fields(&self) -> TokenStream {
        let toasty = &self.toasty;

        self.model
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let index_tokenized = util::int(index);
                let name = &field.name.ident;

                match &field.ty {
                    FieldTy::BelongsTo(_) => {
                        quote!(#name: #toasty::BelongsTo::load(record[#index].take())?,)
                    }
                    FieldTy::HasMany(_) => {
                        quote!(#name: #toasty::HasMany::load(record[#index].take())?,)
                    }
                    FieldTy::HasOne(_) => {
                        quote!(#name: #toasty::HasOne::load(record[#index].take())?,)
                    }
                    FieldTy::Primitive(ty) => {
                        quote!(#name: <#ty as #toasty::stmt::Primitive>::load(record[#index_tokenized].take())?,)
                    }
                }
            })
            .collect()
    }
}
