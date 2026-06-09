use super::{Expand, util};
use crate::model::schema::{FieldTy, ModelKind};

use proc_macro2::TokenStream;
use quote::quote;

impl Expand<'_> {
    pub(super) fn expand_model_impls(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;

        let (
            field_struct_ident,
            field_list_struct_ident,
            query_struct_ident,
            create_struct_ident,
            update_struct_ident,
        ) = match &self.model.kind {
            ModelKind::Root(root) => (
                &root.field_struct_ident,
                &root.field_list_struct_ident,
                &root.query_struct_ident,
                &root.create_struct_ident,
                &root.update_struct_ident,
            ),
            ModelKind::EmbeddedStruct(_) | ModelKind::EmbeddedEnum(_) => {
                // Embedded models don't generate CRUD methods, just return early
                return TokenStream::new();
            }
        };
        let model_schema = self.expand_model_schema();
        let field_register_calls = self.expand_field_register_calls();
        let model_fields = self.expand_model_field_struct_init();
        let load_body = self.expand_load_body(true);
        let filter_methods = self.expand_model_filter_methods();
        let field_name_to_id = self.expand_field_name_to_id();
        let relation_methods = self.expand_model_relation_methods();
        let into_statement_body = self.expand_model_into_statement_body();
        let into_delete_body = self.expand_model_into_delete_body();
        let into_expr_body_ref = self.expand_model_into_expr_body(true);
        let into_expr_body_val = self.expand_model_into_expr_body(false);
        let reload_trait_method = self.expand_reload_trait_method();
        let version_update_stmts = self.expand_version_update_stmts();
        let primary_key_ty = self.expand_primary_key_ty();
        let find_by_primary_key_body = self.expand_find_by_primary_key_body();

        quote! {
            impl #model_ident {
                #model_fields
                #filter_methods
                #relation_methods

                #vis fn create() -> #create_struct_ident {
                    #create_struct_ident::default()
                }

                #vis fn create_many() -> #toasty::stmt::CreateMany<#model_ident> {
                    #toasty::stmt::CreateMany::default()
                }

                #vis fn update(&mut self) -> #update_struct_ident<&mut Self> {
                    let mut s = #update_struct_ident {
                        assignments: #toasty::core::stmt::Assignments::default(),
                        condition: None,
                        target: self,
                    };
                    s.apply_update_defaults();
                    #version_update_stmts
                    s
                }

                #vis fn all() -> #query_struct_ident {
                    #query_struct_ident::default()
                }

                #vis fn filter(expr: #toasty::stmt::Expr<bool>) -> #query_struct_ident {
                    #query_struct_ident::from_stmt(#toasty::stmt::Query::all().filter(expr))
                }

                #vis fn delete(self) -> #toasty::stmt::Delete<()> {
                    #into_delete_body
                }
            }

            #toasty::inventory::submit! {
                #toasty::DiscoverItem::new(
                    env!("CARGO_PKG_NAME"),
                    |model_set| { <#model_ident as #toasty::Model>::register(model_set); },
                )
            }

            impl #toasty::Load for #model_ident {
                type Output = Self;

                fn ty() -> #toasty::core::stmt::Type {
                    #toasty::core::stmt::Type::Model(<Self as #toasty::Model>::id())
                }

                fn load(value: #toasty::core::stmt::Value) -> #toasty::Result<Self> {
                    #load_body
                }

                #reload_trait_method
            }

            impl #toasty::Model for #model_ident {
                type Query<T> = #query_struct_ident<T>;
                type Create = #create_struct_ident;
                type Update<'a> = #update_struct_ident<&'a mut Self>;
                type UpdateQuery = #update_struct_ident;
                type Path<__Origin> = #field_struct_ident<__Origin>;
                type PrimaryKey = #primary_key_ty;
                type ManyField<__Origin> = #field_list_struct_ident<__Origin>;
                type OneField<__Origin> = #field_struct_ident<__Origin>;

                fn id() -> #toasty::core::schema::app::ModelId {
                    static ID: std::sync::OnceLock<#toasty::core::schema::app::ModelId> = std::sync::OnceLock::new();
                    *ID.get_or_init(|| #toasty::generate_unique_id())
                }

                #model_schema

                fn register(model_set: &mut #toasty::core::schema::app::ModelSet) {
                    if model_set.contains(<Self as #toasty::Model>::id()) {
                        return;
                    }
                    model_set.add(<Self as #toasty::Model>::schema());
                    #( #field_register_calls )*
                }

                fn new_path<__Origin>(path: #toasty::Path<__Origin, Self>) -> Self::Path<__Origin> {
                    #field_struct_ident::from_path(path)
                }

                fn new_many_field<__Origin>(
                    path: #toasty::Path<__Origin, #toasty::List<Self>>,
                ) -> #field_list_struct_ident<__Origin> {
                    #field_list_struct_ident::from_path(path)
                }

                fn find_by_primary_key(
                    id: #toasty::stmt::Expr<Self::PrimaryKey>,
                ) -> Self::Query<#toasty::List<Self>> {
                    #find_by_primary_key_body
                }

                fn wrap_query<T>(
                    stmt: #toasty::stmt::Query<T>,
                ) -> Self::Query<T> {
                    #query_struct_ident::from_stmt(stmt)
                }

                fn query_one(
                    query: Self::Query<#toasty::List<Self>>,
                ) -> Self::Query<Self> {
                    query.one()
                }

                fn query_first(
                    query: Self::Query<#toasty::List<Self>>,
                ) -> Self::Query<#toasty::Option<Self>> {
                    query.first()
                }

                #field_name_to_id
            }

            // A relation-terminal `#[has_many(via = …)]` reaching this model
            // keeps its rich per-model query builder. Scalar terminals route
            // through the per-primitive `ViaTarget` impls instead.
            impl #toasty::ViaTarget for #model_ident {
                type Query = #query_struct_ident<#toasty::List<#model_ident>>;
                type Path<__Origin> = <#model_ident as #toasty::Model>::ManyField<__Origin>;

                fn new_path<__Origin>(
                    path: #toasty::Path<__Origin, #toasty::List<#model_ident>>,
                ) -> Self::Path<__Origin> {
                    // A model terminal keeps navigating, so hand back its
                    // chainable `ManyField`.
                    <#model_ident as #toasty::Model>::new_many_field(path)
                }

                fn via_field_ty(
                    singular: #toasty::core::schema::Name,
                    path: #toasty::core::stmt::Path,
                ) -> #toasty::core::schema::app::FieldTy {
                    #toasty::via::model_via_field_ty::<#model_ident>(singular, path)
                }

                fn make_via_query(
                    assoc: #toasty::stmt::Association<#toasty::List<#model_ident>>,
                ) -> Self::Query {
                    <#query_struct_ident<#toasty::List<#model_ident>>>::from_assoc_many(assoc)
                }
            }

            impl #toasty::stmt::IntoExpr<#model_ident> for #model_ident {
                fn into_expr(self) -> #toasty::stmt::Expr<#model_ident> {
                    #into_expr_body_val
                }

                fn by_ref(&self) -> #toasty::stmt::Expr<#model_ident> {
                    #into_expr_body_ref
                }
            }

            impl #toasty::Assign<#model_ident> for #model_ident {
                fn into_assignment(self) -> #toasty::stmt::Assignment<#model_ident> {
                    #toasty::stmt::set(
                        <Self as #toasty::IntoExpr<#model_ident>>::into_expr(self)
                    )
                }
            }

            impl #toasty::IntoStatement for &#model_ident {
                type Returning = #model_ident;

                fn into_statement(self) -> #toasty::Statement<#model_ident> {
                    use #toasty::IntoStatement;
                    #into_statement_body
                }
            }

            impl #toasty::IntoStatement for &mut #model_ident {
                type Returning = #model_ident;

                fn into_statement(self) -> #toasty::Statement<#model_ident> {
                    (&*self).into_statement()
                }
            }

            impl #toasty::IntoStatement for #model_ident {
                type Returning = #model_ident;

                fn into_statement(self) -> #toasty::Statement<#model_ident> {
                    (&self).into_statement()
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

    fn expand_model_into_statement_body(&self) -> TokenStream {
        let toasty = &self.toasty;
        let filter = self.primary_key_filter();
        let query_struct_ident = &self.model.kind.as_root_unwrap().query_struct_ident;
        let filter_method_ident = &filter.filter_method_ident;
        let arg_idents = self.expand_filter_arg_idents(filter);

        quote! {
            #toasty::IntoStatement::into_statement(
                #query_struct_ident::default()
                    .#filter_method_ident( #( & self.#arg_idents ),* )
                    .one()
            )
        }
    }

    fn expand_model_into_delete_body(&self) -> TokenStream {
        let filter = self.primary_key_filter();
        let query_struct_ident = &self.model.kind.as_root_unwrap().query_struct_ident;
        let filter_method_ident = &filter.filter_method_ident;
        let arg_idents = self.expand_filter_arg_idents(filter);

        let version_condition = self.expand_version_delete_condition();

        quote! {
            {
                let __delete = #query_struct_ident::default()
                    .#filter_method_ident( #( & self.#arg_idents ),* )
                    .delete();
                #version_condition
            }
        }
    }

    /// Generate the version condition to set on the delete statement, if the
    /// model has a `#[version]` field. Returns a block that evaluates to
    /// `toasty::stmt::Delete<()>`.
    fn expand_version_delete_condition(&self) -> TokenStream {
        let toasty = &self.toasty;

        let Some(version_index) = self.model.kind.as_root().and_then(|r| r.version_field) else {
            return quote! { __delete };
        };

        let field = &self.model.fields[version_index];
        let index_tokenized = util::int(version_index);
        let field_ident = &field.name.ident;
        let FieldTy::Primitive(field_ty) = &field.ty else {
            unreachable!("version field must be primitive");
        };

        quote! {
            __delete.set_condition(
                #toasty::core::stmt::Condition::new(
                    #toasty::core::stmt::Expr::eq(
                        #toasty::core::stmt::Expr::Reference(
                            #toasty::core::stmt::ExprReference::Field {
                                nesting: 0,
                                index: #index_tokenized,
                            }
                        ),
                        #toasty::into_untyped_expr::<#field_ty, _>(self.#field_ident),
                    )
                )
            )
        }
    }

    /// Emit the Rust type used for `Model::PrimaryKey`: the scalar type for
    /// single-column keys, or a tuple of column types for composite keys.
    fn expand_primary_key_ty(&self) -> TokenStream {
        let pk_fields: Vec<_> = self
            .model
            .primary_key_fields()
            .expect("expand_primary_key_ty called on model without primary key")
            .collect();

        let types: Vec<&syn::Type> = pk_fields
            .iter()
            .map(|field| match &field.ty {
                FieldTy::Primitive(ty) => ty,
                _ => panic!("primary key fields must be primitive"),
            })
            .collect();

        if types.len() == 1 {
            let ty = &types[0];
            quote!(#ty)
        } else {
            quote!(( #( #types ),* ))
        }
    }

    /// Emit the body of `Model::find_by_primary_key`.
    ///
    /// For a single-column PK, delegate to the inherent `filter_by_<pk>` method
    /// (which takes `impl IntoExpr<T>`; `Expr<T>` satisfies that bound).
    ///
    /// For a composite PK, the inherent `filter_by_<pk1>_and_<pk2>` takes
    /// positional args — we only have a single opaque `Expr<(T1, T2)>`, so we
    /// build a record expression of the PK column paths and compare to it
    /// instead. The simplifier decomposes `Record == Record` into per-column
    /// equalities.
    fn expand_find_by_primary_key_body(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        let filter = self.primary_key_filter();
        let pk_fields: Vec<_> = self
            .model
            .primary_key_fields()
            .expect("expand_find_by_primary_key_body called on model without primary key")
            .collect();

        if pk_fields.len() == 1 {
            let filter_method_ident = &filter.filter_method_ident;
            quote! {
                Self::#filter_method_ident(id)
            }
        } else {
            let field_idents = pk_fields.iter().map(|f| &f.name.ident);
            quote! {
                let pk_expr: #toasty::stmt::Expr<Self::PrimaryKey> =
                    #toasty::IntoExpr::into_expr((
                        #( #model_ident::fields().#field_idents() ),*
                    ));
                Self::filter(pk_expr.eq(id))
            }
        }
    }

    pub(super) fn expand_model_into_expr_body(&self, by_ref: bool) -> TokenStream {
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

    pub(super) fn expand_embedded_into_expr_body(
        &self,
        fields_named: bool,
        by_ref: bool,
    ) -> TokenStream {
        let toasty = &self.toasty;

        // For embedded types, create a record expression from all fields
        // Currently only primitive fields are supported in embedded types
        let field_exprs = self.model.fields.iter().enumerate().map(|(index, field)| {
            let ty = match &field.ty {
                FieldTy::Primitive(ty) => ty,
                _ => panic!("only primitive fields are supported in embedded types"),
            };

            let value = if fields_named {
                let field_ident = &field.name.ident;
                if by_ref {
                    quote!((&self.#field_ident))
                } else {
                    quote!(self.#field_ident)
                }
            } else {
                let idx = syn::Index::from(index);
                if by_ref {
                    quote!((&self.#idx))
                } else {
                    quote!(self.#idx)
                }
            };

            // Bind through `Field::ExprTarget` so wrappers such as
            // `Deferred<T>` encode the underlying expression type.
            let target_ty = quote!(FieldExprTarget<#ty>);
            quote!(#toasty::into_untyped_expr::<#target_ty, _>(#value))
        });

        quote! {
            #toasty::stmt::Expr::from_untyped(
                #toasty::core::stmt::Expr::record([
                    #( #field_exprs ),*
                ])
            )
        }
    }

    /// Generates the body for loading a model or embedded type from a Value.
    ///
    /// This method is used by both:
    /// - Root models (in `Load::load`) - supports all field types
    /// - Embedded types (in `Load::load`) - only primitive fields
    ///
    /// The generated code pattern matches on `Value::Record`, extracts fields,
    /// and constructs the struct.
    pub(super) fn expand_load_body(&self, fields_named: bool) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;

        // Generate field loading expressions
        let field_loads = self.model.fields.iter().enumerate().map(|(index, field)| {
            let field_ident = &field.name.ident;
            let index_tokenized = util::int(index);

            let field_name = if fields_named {
                quote!(#field_ident:)
            } else {
                quote!()
            };

            match &field.ty {
                FieldTy::Primitive(ty) => {
                    quote!(#field_name <#ty as #toasty::Load>::load(record[#index_tokenized].take())?,)
                }
                FieldTy::BelongsTo(rel) => {
                    let ty = &rel.ty;
                    quote!(#field_name <#ty as #toasty::Load>::load(record[#index].take())?,)
                }
                FieldTy::HasMany(rel) => {
                    let ty = &rel.ty;
                    quote!(#field_name <#ty as #toasty::Load>::load(record[#index].take())?,)
                }
                FieldTy::HasOne(rel) => {
                    let ty = &rel.ty;
                    quote!(#field_name <#ty as #toasty::Load>::load(record[#index].take())?,)
                }
            }
        });

        let model_load = if fields_named {
            quote!(#model_ident {
                #( #field_loads )*
            })
        } else {
            quote!(#model_ident( #( #field_loads )* ))
        };

        quote! {
            match value {
                #toasty::core::stmt::Value::Null => {
                    Err(#toasty::Error::record_not_found(stringify!(#model_ident)))
                }
                #toasty::core::stmt::Value::Record(mut record) => {
                    Ok(#model_load)
                }
                value => Err(#toasty::Error::type_conversion(value, stringify!(#model_ident))),
            }
        }
    }

    /// Generate the body of the `reload` method for an embedded model's `Primitive` impl.
    ///
    /// Handles two value shapes:
    /// - `SparseRecord` — partial update, reload only the named sub-fields.
    /// - `Record` — whole-embed update, reload every sub-field positionally.
    ///
    /// The positional path matters for embeds with deferred sub-fields:
    /// the assigned record carries the inner T directly (because `IntoExpr<T>`
    /// for `Deferred<T>` unwraps), so each sub-field must go through `reload`
    /// — which knows to re-wrap a bare value as loaded — rather than through
    /// `Load::load`, which expects the SELECT-format `Record([loaded])` for
    /// deferred columns and would reject a bare value.
    pub(super) fn expand_embedded_reload_body(&self, fields_named: bool) -> TokenStream {
        let toasty = &self.toasty;

        let reload_arms: Vec<_> = self
            .model
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let i = util::int(index);

                // For newtypes, access via tuple index (target.0); otherwise by name
                let field_access = if fields_named {
                    let field_ident = &field.name.ident;
                    quote!(target.#field_ident)
                } else {
                    let idx = syn::Index::from(index);
                    quote!(target.#idx)
                };

                match &field.ty {
                    FieldTy::Primitive(ty) => {
                        quote!(#i => <#ty as #toasty::Load>::reload(&mut #field_access, value)?,)
                    }
                    FieldTy::BelongsTo(rel) => {
                        let ty = &rel.ty;
                        quote!(#i => <#ty as #toasty::RelationOneField>::reload(&mut #field_access, value)?,)
                    }
                    FieldTy::HasMany(rel) => {
                        let ty = &rel.ty;
                        if rel.via.is_some() {
                            // A via field has no `RelationManyField` impl (its
                            // element may be a scalar); reload through `Load`.
                            quote!(#i => <#ty as #toasty::Load>::reload(&mut #field_access, value)?,)
                        } else {
                            quote!(#i => <#ty as #toasty::RelationManyField>::reload(&mut #field_access, value)?,)
                        }
                    }
                    FieldTy::HasOne(rel) => {
                        let ty = &rel.ty;
                        quote!(#i => <#ty as #toasty::RelationOneField>::reload(&mut #field_access, value)?,)
                    }
                }
            })
            .collect();

        quote! {
            match value {
                #toasty::core::stmt::Value::SparseRecord(sparse) => {
                    for (field, value) in sparse.into_iter() {
                        match field {
                            #( #reload_arms )*
                            _ => todo!("handle unknown field in embedded reload"),
                        }
                    }
                    Ok(())
                }
                #toasty::core::stmt::Value::Record(record) => {
                    for (field, value) in record.fields.into_iter().enumerate() {
                        match field {
                            #( #reload_arms )*
                            _ => todo!("handle unknown field in embedded reload"),
                        }
                    }
                    Ok(())
                }
                value => {
                    *target = <Self as #toasty::Load>::load(value)?;
                    Ok(())
                }
            }
        }
    }
}
