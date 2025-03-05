use super::{util, Expand};
use crate::schema::FieldTy;

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_impls(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let id = &self.tokenized_id;
        let model_schema = self.expand_model_schema();
        let model_fields = self.expand_model_field_struct_init();
        let fields_struct_ident = &self.model.field_struct_ident;
        let struct_load_fields = self.expand_struct_load_fields();
        let query_struct_ident = &self.model.query_struct_ident;
        let create_builder_ident = &self.model.create_builder_struct_ident;
        let filter_methods = self.expand_model_filter_methods();
        let field_name_to_id = self.expand_field_name_to_id();

        let into_select_body_ref = self.expand_model_into_select_body(true);
        let into_select_body_value = self.expand_model_into_select_body(false);
        let into_expr_body_ref = self.expand_model_into_expr_body(true);
        let into_expr_body_val = self.expand_model_into_expr_body(false);

        quote! {
            impl #model_ident {
                #model_schema
                #model_fields
                #filter_methods

                #vis fn create() -> #create_builder_ident {
                    #create_builder_ident::default()
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
                const ID: #toasty::ModelId = #toasty::ModelId(#id);

                fn load(mut record: #toasty::ValueRecord) -> #toasty::Result<Self> {
                    Ok(Self {
                        #struct_load_fields
                    })
                }
            }

            impl #toasty::Relation for #model_ident {
                const ID: #toasty::ModelId = #toasty::ModelId(#id);
                type Fields = #fields_struct_ident;
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
        let arg_idents = self.expand_filter_arg_idents(&filter);
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
                    /*
                    FieldTy::HasMany(_) => {
                        quote!(#name: HasMany::load(record[#index].take())?,)
                    }
                    FieldTy::HasOne(_) => quote!(),
                    */
                    FieldTy::BelongsTo(_) => {
                        quote!(#name: #toasty::BelongsTo::load(record[#index].take())?,)
                    }
                    FieldTy::Primitive(ty) => {
                        quote!(#name: <#ty as #toasty::stmt::Primitive>::load(record[#index_tokenized].take()),)
                    }
                }
            })
            .collect()
    }
}
