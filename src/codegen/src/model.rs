mod body;
mod create;
mod fields;
mod filters;
mod query;
mod relation;
mod update;

use filters::Filter;

use crate::*;

use toasty_core::{schema::*, stmt};

use proc_macro2::TokenStream;
use quote::quote;
use std::rc::Rc;

pub(crate) fn generate(model: &app::Model, names: Rc<Names>, in_macro: bool) -> ModelOutput<'_> {
    let mut gen = Generator::new(model, names, in_macro);
    let module_name = gen.names.models[&model.id].module_name.clone();

    ModelOutput {
        model,
        module_name,
        body: gen.gen_model_body(),
    }
}

/// Generate the Rust Toasty client for the specified model.
pub(crate) struct Generator<'a> {
    /// Model being generated
    model: &'a app::Model,

    /// Model filters to generate methods for
    filters: Vec<Filter>,

    /// Stores various names
    names: Rc<Names>,

    /// Whether or not generating code from within a macro
    in_macro: bool,
}

impl<'a> Generator<'a> {
    /// Create a new `GenModel` for the provided model
    pub(crate) fn new(model: &'a app::Model, names: Rc<Names>, in_macro: bool) -> Generator<'a> {
        let filters = Filter::build_model_filters(model);

        Generator {
            model,
            filters,
            names,
            in_macro,
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

    pub(crate) fn module_name(&self, id: app::ModelId, depth: usize) -> TokenStream {
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

    pub(crate) fn field_name(&self, field: impl Into<app::FieldId>) -> &syn::Ident {
        let field = field.into();
        &self.names.fields[&field].field_name
    }

    pub(crate) fn field_const_name(&self, field: impl Into<app::FieldId>) -> &syn::Ident {
        let field = field.into();
        &self.names.fields[&field].const_name
    }

    pub(crate) fn singular_name(&self, field: impl Into<app::FieldId>) -> &syn::Ident {
        let field = field.into();
        self.names.relations[&field].singular_name.as_ref().unwrap()
    }

    pub(crate) fn field_ty(&self, field: impl Into<app::FieldId>, depth: usize) -> TokenStream {
        use app::FieldTy::*;

        let field = field.into();
        assert_eq!(field.model, self.model.id);

        let field = &self.model.fields[field.index];

        match &field.ty {
            Primitive(field_ty) => self.ty(&field_ty.ty, depth),
            BelongsTo(_) | HasOne(_) | HasMany(_) => {
                let module_name = self.module_name(field.id.model, depth);
                let relation_struct_name = self.relation_struct_name(field.id());

                quote! {
                    #module_name::relation::#relation_struct_name
                }
            }
        }
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

    pub(crate) fn model_struct_path(
        &self,
        id: impl Into<app::ModelId>,
        depth: usize,
    ) -> TokenStream {
        let id = id.into();
        let name = &self.names.models[&id].struct_name;

        if id == self.model.id {
            quote!(#name)
        } else {
            let target_mod_name = self.module_name(id, depth);
            quote!(#target_mod_name::#name)
        }
    }

    pub(crate) fn target_struct_path(
        &self,
        field: impl Into<app::FieldId>,
        depth: usize,
    ) -> TokenStream {
        let field = field.into();

        assert!(field.model == self.model.id);

        let target = match &self.model.fields[field.index].ty {
            app::FieldTy::HasOne(rel) => rel.target,
            app::FieldTy::HasMany(rel) => rel.target,
            app::FieldTy::BelongsTo(rel) => rel.target,
            app::FieldTy::Primitive(_) => unreachable!(),
        };

        self.model_struct_path(target, depth)
    }

    pub(crate) fn relation_struct_name(&self, field: impl Into<app::FieldId>) -> &syn::Ident {
        let field = field.into();
        &self.names.relations[&field].struct_name
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
