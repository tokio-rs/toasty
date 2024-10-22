mod body;
mod create;
mod fields;
mod find_by;
mod query;
mod relation;
mod update;

use crate::*;

use toasty_core::{schema::*, stmt};

use proc_macro2::TokenStream;
use quote::quote;
use std::rc::Rc;

pub(crate) fn generate<'a>(
    schema: &'a Schema,
    model: &'a Model,
    names: Rc<Names>,
    in_macro: bool,
) -> ModelOutput<'a> {
    let mut gen = Generator::new(schema, model, names, in_macro);
    let module_name = gen.names.models[&model.id].module_name.clone();

    ModelOutput {
        model,
        module_name,
        body: gen.gen_body(),
    }
}

/// Generate the Rust Toasty client for the specified model.
pub(crate) struct Generator<'a> {
    pub schema: &'a Schema,

    /// Model being generated
    pub model: &'a Model,

    /// Stores various names
    pub names: Rc<Names>,

    /// Whether or not generating code from within a macro
    pub in_macro: bool,
}

impl<'a> Generator<'a> {
    /// Create a new `GenModel` for the provided model
    pub(crate) fn new(
        schema: &'a Schema,
        model: &'a Model,
        names: Rc<Names>,
        in_macro: bool,
    ) -> Generator<'a> {
        Generator {
            schema,
            model,
            names,
            in_macro,
        }
    }

    pub(crate) fn module_path(&self, mid: ModelId, depth: usize) -> TokenStream {
        if mid == self.model.id {
            quote!()
        } else {
            let path = self.module_name(mid, depth);
            quote!(#path::)
        }
    }

    pub(crate) fn mod_prefix(&self, depth: usize) -> TokenStream {
        if self.in_macro {
            let mut ret = quote!(super);

            for _ in 0..depth {
                ret = quote!(#ret::super);
            }

            ret
        } else {
            let name = self
                .names
                .container_alias_name
                .as_ref()
                .unwrap_or(&self.names.container_module_name);

            quote!(#name)
        }
    }

    pub(crate) fn module_name(&self, id: ModelId, depth: usize) -> TokenStream {
        let name = &self.names.models[&id].module_name;

        if id == self.model.id {
            if depth == 0 {
                quote!(self)
            } else {
                quote!(#name)
            }
        } else {
            let prefix = self.mod_prefix(depth);
            quote!(#prefix::#name)
        }
    }

    pub(crate) fn field_name(&self, field: impl Into<FieldId>) -> &syn::Ident {
        let field = field.into();
        &self.names.fields[&field].field_name
    }

    pub(crate) fn field_const_name(&self, field: impl Into<FieldId>) -> &syn::Ident {
        let field = field.into();
        &self.names.fields[&field].const_name
    }

    pub(crate) fn singular_name(&self, field: impl Into<FieldId>) -> &syn::Ident {
        let field = field.into();
        &self.names.relations[&field].singular_name.as_ref().unwrap()
    }

    pub(crate) fn field_ty(&self, field: &Field, depth: usize) -> TokenStream {
        use FieldTy::*;

        match &field.ty {
            Primitive(field_ty) => self.ty(&field_ty.ty, depth),
            BelongsTo(_) | HasOne(_) | HasMany(_) => {
                let module_name = self.module_name(field.id.model, depth);
                let relation_struct_name = self.relation_struct_name(field);

                quote! {
                    #module_name::relation::#relation_struct_name
                }
            }
        }
    }

    pub(crate) fn query(&self, id: impl Into<QueryId>) -> &Query {
        self.schema.query(id.into())
    }

    pub(crate) fn pk_query(&self) -> &Query {
        self.schema.query(self.model.primary_key.query)
    }

    pub(crate) fn ty(&self, ty: &stmt::Type, depth: usize) -> TokenStream {
        match ty {
            stmt::Type::Bool => quote!(bool),
            stmt::Type::Id(model) => {
                let struct_name = self.model_struct_path(*model, depth);
                quote!(Id<#struct_name>)
            }
            stmt::Type::String => quote!(String),
            stmt::Type::I64 => quote!(i64),
            _ => todo!("ty = {:#?}", ty),
        }
    }

    pub(crate) fn self_struct_name(&self) -> TokenStream {
        self.model_struct_path(self.model.id, 1)
    }

    pub(crate) fn model_struct_path(&self, id: impl Into<ModelId>, depth: usize) -> TokenStream {
        let id = id.into();
        let name = &self.names.models[&id].struct_name;

        if id == self.model.id {
            quote!(#name)
        } else {
            let target_mod_name = self.module_name(id, depth);
            quote!(#target_mod_name::#name)
        }
    }

    pub(crate) fn model_field_count(&self) -> TokenStream {
        util::int(self.model.fields.len())
    }

    pub(crate) fn relation_struct_name(&self, field: impl Into<FieldId>) -> &syn::Ident {
        let field = field.into();
        &self.names.relations[&field].struct_name
    }

    pub(crate) fn relation_query_struct_path(
        &self,
        field: impl Into<FieldId>,
        depth: usize,
    ) -> TokenStream {
        let field = field.into();
        let target_mod_name = self.module_name(field.model, depth);
        let field_name = self.field_name(field);
        quote!(#target_mod_name::relation::#field_name::Query)
    }

    pub(crate) fn model_pk_query_method_name(&self, id: ModelId) -> &syn::Ident {
        let query = self.schema.model(id).primary_key.query;
        self.query_method_name(query)
    }

    pub(crate) fn query_method_name(&self, query: QueryId) -> &syn::Ident {
        &self.names.queries[&query].method_name
    }

    pub(crate) fn scoped_query_method_name(&self, query: QueryId) -> &syn::Ident {
        &self.names.queries[&query]
            .scoped_method_name
            .as_ref()
            .unwrap()
    }

    pub(crate) fn query_struct_name(&self, query: QueryId) -> &syn::Ident {
        &self.names.queries[&query].struct_name
    }

    pub(crate) fn container_import(&self) -> TokenStream {
        if self.in_macro {
            quote!()
        } else {
            let name = &self.names.container_module_name;
            let alias = &self.names.container_alias_name;

            if let Some(alias) = alias {
                quote!(use super::super::#name as #alias;)
            } else {
                quote!(use super::super::#name;)
            }
        }
    }

    // Replace `load_primitive_from` with this?
    pub(crate) fn primitive_from_value(
        &self,
        ty: &stmt::Type,
        nullable: bool,
        from: TokenStream,
    ) -> TokenStream {
        match ty {
            stmt::Type::Id(_) => {
                if nullable {
                    quote! {
                        #from.to_option_id()?.map(Id::from_untyped)
                    }
                } else {
                    quote!(Id::from_untyped(#from.to_id()?))
                }
            }
            _ => {
                let convert = self.value_to_ty_fn(ty, nullable);

                quote! {
                    #from.#convert()?
                }
            }
        }
    }

    pub(crate) fn value_to_ty_fn(&self, ty: &stmt::Type, nullable: bool) -> TokenStream {
        if nullable {
            match ty {
                stmt::Type::String => quote!(to_option_string),
                stmt::Type::Id(_) => quote!(to_option_id),
                _ => todo!("ty={:#?}", ty),
            }
        } else {
            match ty {
                stmt::Type::Bool => quote!(to_bool),
                stmt::Type::Id(_) => quote!(to_id),
                stmt::Type::String => quote!(to_string),
                stmt::Type::I64 => quote!(to_i64),
                _ => todo!("ty={:#?}", ty),
            }
        }
    }
}
