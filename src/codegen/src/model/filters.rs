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
        let iter = self.model.primary_key_fields();

        if iter.len() == 1 {
            let expr = iter.map(|field| {
                let name = self.field_name(field);

                if by_ref {
                    quote!((&self.#name).into_expr().cast())
                } else {
                    quote!(self.#name.into_expr().cast())
                }
            });

            quote!( #( #expr )* )
        } else {
            let expr = iter.map(|field| {
                let name = self.field_name(field);
                let amp = if by_ref { quote!(&) } else { quote!() };
                quote!( #amp self.#name)
            });

            quote!( ( #( #expr ),* ).into_expr().cast() )
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

/*
pub(super) fn gen_query_structs(&mut self) -> TokenStream {
    self.model
        .queries
        .iter()
        .copied()
        .map(|query| self.find_by_query(self.query(query)))
        .collect()
}

pub(crate) fn gen_scoped_find_by_method(&self, scoped: &app::ScopedQuery) -> TokenStream {
    let query_method_name = self.scoped_query_method_name(scoped.id);
    let query_struct_name = self.query_struct_name(scoped.id);

    let caller_idents: Vec<_> = scoped
        .caller_args
        .iter()
        .map(|arg| {
            let ident = crate::util::ident(&arg.name);
            quote!(#ident)
        })
        .collect();

    let mut idents = vec![];

    for _ in &scoped.scope_args {
        idents.push(quote!(self.scope));
    }

    idents.extend(caller_idents.clone());

    let query_method_args = self.gen_method_args(&scoped.caller_args, &caller_idents, 2);

    let query = self.query(scoped.id);
    let filter = &query.stmt.body.as_select().filter;
    let body = self.gen_expr_from_stmt(query.ret, &idents, filter, 2);

    quote! {
        // TODO: should this borrow more?
        pub fn #query_method_name(self, #query_method_args) ->  #query_struct_name {
            #query_struct_name {
                stmt: stmt::Select::filter(#body)
            }
        }
    }
}

// TODO: split this up and unify with other fns
pub(crate) fn gen_scoped_find_by_struct(
    &self,
    scoped: &app::ScopedQuery,
    depth: usize,
) -> TokenStream {
    let query = self.query(scoped.id);
    self.gen_find_by_struct(query, depth)
}

fn find_by_query(&self, query: &app::Query) -> TokenStream {
    if !query.many {
        self.find_basic_by_query(query)
    } else {
        self.find_many_by_query(query)
    }
}

fn find_basic_by_query(&self, query: &app::Query) -> TokenStream {
    use app::FieldTy;

    let query_method_name = self.query_method_name(query.id);
    let model_struct_name = self.self_struct_name();
    let query_struct_name = self.query_struct_name(query.id);

    let arg_idents: Vec<_> = query
        .args
        .iter()
        .map(|arg| {
            let ident = crate::util::ident(&arg.name);
            quote!(#ident)
        })
        .collect();

    let query_method_args = self.gen_method_args(&query.args, &arg_idents, 1);

    let filter = &query.stmt.body.as_select().filter;
    let query = self.gen_expr_from_stmt(query.ret, &arg_idents, filter, 1);

    let relation_methods = self
        .model
        .fields
        .iter()
        .filter_map(|field| match &field.ty {
            FieldTy::Primitive(..) => None,
            FieldTy::HasMany(_) | FieldTy::HasOne(_) => {
                let name = self.field_name(field.id);
                let relation_query_struct_path = self.relation_query_struct_path(field, 0);

                Some(quote! {
                    pub fn #name(mut self) -> #relation_query_struct_path {
                        #relation_query_struct_path::with_scope(self)
                    }
                })
            }
            FieldTy::BelongsTo(_) => None,
        });

    quote! {
        impl super::#model_struct_name {
            // TODO: should this borrow more?
            pub fn #query_method_name(#query_method_args) ->  #query_struct_name {
                #query_struct_name {
                    query: Query::from_stmt(stmt::Select::filter(#query))
                }
            }
        }

        pub struct #query_struct_name {
            query: Query,
        }

        impl #query_struct_name {
            pub async fn all(self, db: &Db) -> Result<Cursor<super::#model_struct_name>> {
                self.query.all(db).await
            }

            pub async fn first(self, db: &Db) -> Result<Option<super::#model_struct_name>> {
                self.query.first(db).await
            }

            pub async fn get(self, db: &Db) -> Result<super::#model_struct_name> {
                self.query.get(db).await
            }

            pub fn update(self) -> super::UpdateQuery {
                super::UpdateQuery::from(self.query)
            }

            pub async fn delete(self, db: &Db) -> Result<()> {
                self.query.delete(db).await
            }

            pub fn include<T: ?Sized>(mut self, path: impl Into<Path<T>>) -> #query_struct_name {
                let path = path.into();
                self.query.stmt.include(path);
                self
            }

            pub fn filter(self, filter: stmt::Expr<bool>) -> Query {
                let stmt = self.into_select();
                Query::from_stmt(stmt.and(filter))
            }

            pub async fn collect<A>(self, db: &Db) -> Result<A>
            where
                A: FromCursor<super::#model_struct_name>
            {
                self.all(db).await?.collect().await
            }

            #( #relation_methods )*
        }

        impl stmt::IntoSelect for #query_struct_name {
            type Model = super::#model_struct_name;

            fn into_select(self) -> stmt::Select<Self::Model> {
                self.query.into_select()
            }
        }
    }
}

fn find_many_by_query(&self, query: &app::Query) -> TokenStream {
    let query_method_name = self.query_method_name(query.id);
    let model_struct_name = self.self_struct_name();
    let query_struct_name = self.query_struct_name(query.id);

    let arg_idents: Vec<_> = query
        .args
        .iter()
        .map(|arg| {
            let ident = crate::util::ident(&arg.name);
            quote!(#ident)
        })
        .collect();

    let item_ty = self.gen_item_arg_tys(&query.args);

    let query_push_item = if query.args.len() == 1 {
        let ident = &arg_idents[0];
        quote!( #ident.into_expr() )
    } else {
        quote!(( #( #arg_idents .into_expr() ),* ).into_expr())
    };

    let query_method_args = self.gen_method_args(&query.args, &arg_idents, 1);

    let filter = &query.stmt.body.as_select().filter;
    let arg_idents = vec![quote!(self.items)];
    let query = self.gen_expr_from_stmt(query.ret, &arg_idents, filter, 1);

    quote! {
        impl super::#model_struct_name {
            // TODO: should this borrow more?
            pub fn #query_method_name() ->  #query_struct_name {
                #query_struct_name { items: vec![] }
            }
        }

        pub struct #query_struct_name {
            items: Vec<stmt::Expr<#item_ty>>,
        }

        impl #query_struct_name {
            pub fn item(mut self, #query_method_args ) -> Self {
                self.items.push( #query_push_item );
                self
            }

            pub async fn all(self, db: &Db) -> Result<Cursor<super::#model_struct_name>> {
                db.all(self.into_select()).await
            }

            pub async fn first(self, db: &Db) -> Result<Option<super::#model_struct_name>> {
                db.first(self.into_select()).await
            }

            pub async fn get(self, db: &Db) -> Result<super::#model_struct_name> {
                db.get(self.into_select()).await
            }

            pub fn update(self) -> super::UpdateQuery {
                super::UpdateQuery::from(self.into_select())
            }

            pub async fn delete(self, db: &Db) -> Result<()> {
                db.delete(self.into_select()).await
            }

            pub fn filter(self, filter: stmt::Expr<bool>) -> Query {
                let stmt = self.into_select();
                Query::from_stmt(stmt.and(filter))
            }

            pub async fn collect<A>(self, db: &Db) -> Result<A>
            where
                A: FromCursor<super::#model_struct_name>
            {
                self.all(db).await?.collect().await
            }

            // #( #relation_methods )*
        }

        impl stmt::IntoSelect for #query_struct_name {
            type Model = super::#model_struct_name;

            fn into_select(self) -> stmt::Select<Self::Model> {
                stmt::Select::filter(#query)
            }
        }
    }
}

fn gen_method_args(
    &self,
    args: &[app::Arg],
    arg_idents: &[TokenStream],
    depth: usize,
) -> TokenStream {
    let args = args.iter().enumerate().map(move |(i, arg)| {
        let name = &arg_idents[i];

        match &arg.ty {
            stmt::Type::Model(model_id) => {
                let target_struct_name = self.model_struct_path(*model_id, depth);

                quote!(
                    #name: impl stmt::IntoExpr<#target_struct_name>
                )
            }
            stmt::Type::ForeignKey(field_id) => {
                let field_name = self.field_name(field_id);
                let relation_struct_name = self.relation_struct_name(field_id);

                quote!(
                    #name: impl stmt::IntoExpr<super::relation::#field_name::#relation_struct_name>
                )
            }
            ty => {
                let ty = self.ty(ty, depth);

                quote! {
                    #name: impl stmt::IntoExpr<#ty>
                }
            }
        }
    });

    quote!( #( #args ),* )
}

fn gen_item_arg_tys(&self, args: &[app::Arg]) -> TokenStream {
    let mut tys = args.iter().map(move |arg| match &arg.ty {
        stmt::Type::Model(model) => {
            let target_struct_name = self.model_struct_path(*model, 1);
            quote!( #target_struct_name )
        }
        stmt::Type::ForeignKey(field_id) => {
            let field_name = self.field_name(field_id);
            let relation_struct_name = self.relation_struct_name(field_id);

            quote!( super::relation::#field_name::#relation_struct_name )
        }
        ty => {
            let ty = self.ty(ty, 1);
            quote!(#ty)
        }
    });

    if tys.len() == 1 {
        let ty = tys.next().unwrap();
        quote!(#ty)
    } else {
        quote!( ( #( #tys ),* ) )
    }
}

fn gen_find_by_struct(&self, query: &app::Query, depth: usize) -> TokenStream {
    let path = self.module_path(query.ret, depth);
    let model_struct_name = self.model_struct_path(query.ret, depth);
    let query_struct_name = self.query_struct_name(query.id);

    quote! {
        pub struct #query_struct_name {
            stmt: stmt::Select<#model_struct_name>,
        }

        impl #query_struct_name {
            pub async fn all(self, db: &Db) -> Result<Cursor<#model_struct_name>> {
                db.all(self.stmt).await
            }

            pub async fn first(self, db: &Db) -> Result<Option<#model_struct_name>> {
                db.first(self.stmt).await
            }

            pub async fn get(self, db: &Db) -> Result<#model_struct_name> {
                db.get(self.stmt).await
            }

            pub fn update(self) -> #path UpdateQuery {
                #path UpdateQuery::from(self.stmt)
            }

            pub async fn delete(self, db: &Db) -> Result<()> {
                db.exec(self.stmt.delete()).await?;
                Ok(())
            }
        }

        impl stmt::IntoSelect for #query_struct_name {
            type Model = #model_struct_name;

            fn into_select(self) -> stmt::Select<Self::Model> {
                self.stmt
            }
        }
    }
}

fn gen_expr_from_stmt(
    &self,
    mid: app::ModelId,
    args: &[TokenStream],
    filter: &stmt::Expr,
    depth: usize,
) -> TokenStream {
    let struct_name = self.model_struct_path(mid, depth);

    match filter {
        stmt::Expr::And(exprs) => self.gen_expr_chain(mid, args, exprs, quote!(and), depth),
        stmt::Expr::Or(exprs) => self.gen_expr_chain(mid, args, exprs, quote!(or), depth),
        stmt::Expr::Field(expr_field) => {
            let base = quote!(#struct_name);
            let field = self.field_const_name(expr_field.field);
            quote!( #base :: #field )
        }
        stmt::Expr::Arg(arg) => {
            let arg = &args[arg.position];
            quote!(#arg)
        }
        stmt::Expr::Value(value) => match value {
            stmt::Value::Bool(v) => quote!(#v),
            stmt::Value::String(s) => quote!(#s),
            _ => todo!(),
        },
        stmt::Expr::Record(exprs) => {
            let exprs = exprs
                .iter()
                .map(|expr| self.gen_expr_from_stmt(mid, args, expr, depth));
            quote!(( #( #exprs ),* ))
        }
        stmt::Expr::List(exprs) => {
            let exprs = exprs
                .items
                .iter()
                .map(|expr| self.gen_expr_from_stmt(mid, args, expr, depth));
            quote!(vec![ #( #exprs ),* ])
        }
        stmt::Expr::BinaryOp(expr_binary_op) => {
            let lhs = self.gen_expr_from_stmt(mid, args, &expr_binary_op.lhs, depth);
            let rhs = self.gen_expr_from_stmt(mid, args, &expr_binary_op.rhs, depth);

            let op = match expr_binary_op.op {
                stmt::BinaryOp::Eq => quote!(eq),
                _ => todo!(),
            };

            quote!(#lhs . #op ( #rhs ))
        }
        stmt::Expr::InList(expr_in_list) => {
            let lhs = self.gen_expr_from_stmt(mid, args, &expr_in_list.expr, depth);
            let rhs = self.gen_expr_from_stmt(mid, args, &expr_in_list.list, depth);
            quote!(stmt::in_set( #lhs, #rhs ))
        }
        stmt::Expr::InSubquery(expr_in_subquery) => {
            let lhs = self.gen_expr_from_stmt(mid, args, &expr_in_subquery.expr, depth);
            let filter = &expr_in_subquery.query.body.as_select().filter;

            let subquery = match filter {
                stmt::Expr::Arg(arg) => {
                    let arg = &args[arg.position];
                    quote!(#arg)
                }
                _ => todo!("expr = {:#?}", filter),
            };

            quote!(#lhs . in_query ( #subquery ))
        }
        expr => todo!("expr={expr:#?}"),
    }
}

fn gen_expr_chain(
    &self,
    model_id: app::ModelId,
    args: &[TokenStream],
    exprs: &[stmt::Expr],
    f: TokenStream,
    depth: usize,
) -> TokenStream {
    assert!(exprs.len() > 1);

    let [head, rest @ ..] = &exprs[..] else {
        panic!()
    };

    let mut expr = self.gen_expr_from_stmt(model_id, args, head, depth);

    for next in rest {
        let next = self.gen_expr_from_stmt(model_id, args, next, depth);
        expr = quote!( #expr . #f ( #next ));
    }

    expr
}
*/
