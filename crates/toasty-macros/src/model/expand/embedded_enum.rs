use super::{Expand, schema, util};
use crate::model::schema::{EnumStorageStrategy, FieldTy, Name, VariantValue};

use hashbrown::HashMap;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

impl Expand<'_> {
    /// Returns fields belonging to a specific variant index.
    fn variant_fields(&self, variant_index: usize) -> Vec<&crate::model::schema::Field> {
        self.model
            .fields
            .iter()
            .filter(|f| f.variant == Some(variant_index))
            .collect()
    }

    /// True when at least one variant carries data fields, which changes
    /// what `Field::ty()` returns.
    pub(super) fn expand_enum_has_data_variants(&self) -> bool {
        !self.model.fields.is_empty()
    }

    fn uses_string_discriminants(&self) -> bool {
        self.model
            .kind
            .as_embedded_enum_unwrap()
            .uses_string_discriminants()
    }

    /// Implements compatibility with every integer storage marker that can
    /// represent all of this enum's discriminants. This lets field-level
    /// overrides fail during type checking, including overrides on transparent
    /// wrappers and `Vec<unit-enum>` fields whose app-schema type no longer
    /// carries the enum model id.
    pub(super) fn expand_enum_discriminant_compat_impls(&self) -> TokenStream {
        if self.uses_string_discriminants() {
            return quote! {};
        }

        let toasty = &self.toasty;
        let model_ident = &self.model.ident;

        let max_discriminant = self
            .model
            .kind
            .as_embedded_enum_unwrap()
            .variants
            .iter()
            .map(|variant| match variant.attrs.discriminant {
                VariantValue::Integer(value) => value,
                VariantValue::String(_) => unreachable!("integer enum has a string discriminant"),
            })
            .max()
            .unwrap_or(0);

        let mut markers = Vec::new();
        if max_discriminant <= i8::MAX.into() {
            markers.push(quote! { #toasty::storage::tag::I8 });
        }
        if max_discriminant <= i16::MAX.into() {
            markers.push(quote! { #toasty::storage::tag::I16 });
        }
        if max_discriminant <= i32::MAX.into() {
            markers.push(quote! { #toasty::storage::tag::I32 });
        }
        markers.push(quote! { #toasty::storage::tag::I64 });
        if max_discriminant <= u8::MAX.into() {
            markers.push(quote! { #toasty::storage::tag::U8 });
        }
        if max_discriminant <= u16::MAX.into() {
            markers.push(quote! { #toasty::storage::tag::U16 });
        }
        if max_discriminant <= i64::from(u32::MAX) {
            markers.push(quote! { #toasty::storage::tag::U32 });
        }
        markers.push(quote! { #toasty::storage::tag::U64 });

        for size in [3_u8, 5, 6, 7] {
            let signed_max = (1_i64 << (u32::from(size) * 8 - 1)) - 1;
            if max_discriminant <= signed_max {
                markers.push(quote! { #toasty::storage::tag::Int<#size> });
            }

            let unsigned_max = (1_i64 << (u32::from(size) * 8)) - 1;
            if max_discriminant <= unsigned_max {
                markers.push(quote! { #toasty::storage::tag::UInt<#size> });
            }
        }

        quote! {
            #(
                impl #toasty::storage::CompatibleWith<#markers> for #model_ident {}
            )*
        }
    }

    /// Generates tokens for an `is_variant(path, variant_id)` expression.
    /// Reused by `is_{variant}()` methods and the `matches()` method.
    fn expand_is_variant_expr(&self, variant_idx: &TokenStream) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        quote! {
            {
                let path_stmt: #toasty::core::stmt::Expr = {
                    let p: #toasty::core::stmt::Path = self.path().into();
                    p.into_stmt()
                };
                let variant_id = #toasty::core::schema::app::VariantId {
                    model: <#model_ident as #toasty::Embed>::id(),
                    index: #variant_idx,
                };
                #toasty::stmt::Expr::from_untyped(
                    #toasty::core::stmt::Expr::is_variant(path_stmt, variant_id)
                )
            }
        }
    }

    /// Generates delegated comparison methods (`eq`, `ne`, `in_list`) that
    /// forward to `self.path()`. Ordered comparisons (`gt`, `ge`, `lt`, `le`)
    /// are intentionally excluded because enums have no meaningful ordering.
    fn expand_comparison_methods(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;

        let methods = ["eq", "ne"].iter().map(|name| {
            let method_ident = syn::Ident::new(name, proc_macro2::Span::call_site());
            quote! {
                #vis fn #method_ident(&self, rhs: impl #toasty::stmt::IntoExpr<#model_ident>) -> #toasty::stmt::Expr<bool> {
                    self.path().#method_ident(rhs)
                }
            }
        });

        quote! {
            #( #methods )*

            #vis fn in_list(&self, rhs: impl #toasty::stmt::IntoExpr<#toasty::List<#model_ident>>) -> #toasty::stmt::Expr<bool> {
                self.path().in_list(rhs)
            }
        }
    }

    /// Generates the `{Enum}Fields` struct for embedded enums with
    /// `is_{variant}()` methods, variant accessor methods, and delegated
    /// comparison methods. Also generates per-variant field structs for
    /// data-carrying variants (e.g., `ContactInfoEmailFields`).
    pub(super) fn expand_enum_field_struct(&self) -> TokenStream {
        let toasty = &self.toasty;
        let vis = &self.model.vis;
        let model_ident = &self.model.ident;
        let embedded_enum = self.model.kind.as_embedded_enum_unwrap();
        let field_struct_ident = &embedded_enum.field_struct_ident;

        let is_variant_methods: Vec<_> = embedded_enum
            .variants
            .iter()
            .enumerate()
            .map(|(variant_index, variant)| {
                let method_name = &variant.is_method_ident;
                let variant_idx = util::int(variant_index);
                let is_variant_check = self.expand_is_variant_expr(&variant_idx);

                quote! {
                    #vis fn #method_name(&self) -> #toasty::stmt::Expr<bool> {
                        #is_variant_check
                    }
                }
            })
            .collect();

        // Generate variant accessor methods for data-carrying variants.
        let variant_accessor_methods: Vec<_> = embedded_enum
            .variants
            .iter()
            .filter(|v| v.variant_handle_ident.is_some())
            .map(|variant| {
                let method_name = &variant.name.ident;
                let variant_handle_ident = variant.variant_handle_ident.as_ref().unwrap();

                quote! {
                    #vis fn #method_name(&self) -> #variant_handle_ident<__Origin> {
                        #variant_handle_ident {
                            path: self.path()
                        }
                    }
                }
            })
            .collect();

        // Generate one struct per data-carrying variant. The same struct is
        // both the entry point returned by `email()` (used for `.matches(...)`
        // filters and direct `.eq()`/include path access) and the namespace
        // for variant-field accessors. The struct stores the parent path
        // (the path to the embed enum field) and exposes:
        //
        // - `parent_path()` — model-rooted path to the enum field. Used by
        //   `matches()` to build the `is_variant` gate and by every filter
        //   method on a variant-rooted Path to inject the implicit gate.
        // - `path()` — variant-rooted path. Field accessors chain off this
        //   so that the produced Path knows its variant context.
        let variant_field_structs: Vec<_> = embedded_enum
            .variants
            .iter()
            .enumerate()
            .filter(|(_, v)| v.variant_handle_ident.is_some())
            .map(|(variant_index, variant)| {
                let variant_handle_ident = variant.variant_handle_ident.as_ref().unwrap();
                let variant_idx = util::int(variant_index);

                let field_methods: Vec<_> = self
                    .variant_fields(variant_index)
                    .iter()
                    .enumerate()
                    .map(|(field_index, field)| {
                        let field_ident = &field.name.ident;
                        let field_ty = primitive_ty_unwrap(field);
                        let field_offset = util::int(field_index);
                        self.expand_primitive_field_method(field_ident, field_ty, &field_offset)
                    })
                    .collect();

                quote! {
                    #vis struct #variant_handle_ident<__Origin> {
                        path: #toasty::Path<__Origin, #model_ident>,
                    }

                    impl<__Origin> #variant_handle_ident<__Origin> {
                        fn parent_path(&self) -> #toasty::Path<__Origin, #model_ident> {
                            self.path.clone()
                        }

                        fn path(&self) -> #toasty::Path<__Origin, #model_ident> {
                            let variant_id = #toasty::core::schema::app::VariantId {
                                model: <#model_ident as #toasty::Embed>::id(),
                                index: #variant_idx,
                            };
                            self.parent_path().into_variant(variant_id)
                        }

                        #vis fn matches(
                            self,
                            f: impl FnOnce(Self) -> #toasty::stmt::Expr<bool>,
                        ) -> #toasty::stmt::Expr<bool> {
                            let parent_stmt: #toasty::core::stmt::Expr = {
                                let p: #toasty::core::stmt::Path = self.parent_path().into();
                                p.into_stmt()
                            };
                            let variant_id = #toasty::core::schema::app::VariantId {
                                model: <#model_ident as #toasty::Embed>::id(),
                                index: #variant_idx,
                            };
                            let is_var = #toasty::stmt::Expr::from_untyped(
                                #toasty::core::stmt::Expr::is_variant(parent_stmt, variant_id)
                            );
                            let body = f(self);
                            is_var.and(body)
                        }

                        #( #field_methods )*
                    }
                }
            })
            .collect();

        let comparison_methods = self.expand_comparison_methods();

        quote! {
            #vis struct #field_struct_ident<__Origin> {
                path: #toasty::Path<__Origin, #model_ident>,
            }

            impl<__Origin> #field_struct_ident<__Origin> {
                fn path(&self) -> #toasty::Path<__Origin, #model_ident> {
                    self.path.clone()
                }

                #( #is_variant_methods )*

                #( #variant_accessor_methods )*

                #comparison_methods
            }

            impl<__Origin> Into<#toasty::Path<__Origin, #model_ident>> for #field_struct_ident<__Origin> {
                fn into(self) -> #toasty::Path<__Origin, #model_ident> {
                    self.path
                }
            }

            impl<__Origin> #toasty::IntoExpr<#model_ident> for #field_struct_ident<__Origin> {
                fn into_expr(self) -> #toasty::stmt::Expr<#model_ident> {
                    use #toasty::IntoExpr;
                    self.path.into_expr()
                }

                fn by_ref(&self) -> #toasty::stmt::Expr<#model_ident> {
                    use #toasty::IntoExpr;
                    self.path.by_ref()
                }
            }

            impl<__Origin> Into<#toasty::stmt::Include<__Origin, #model_ident>> for #field_struct_ident<__Origin> {
                fn into(self) -> #toasty::stmt::Include<__Origin, #model_ident> {
                    self.path.into()
                }
            }

            #( #variant_field_structs )*
        }
    }

    /// Generates the `EnumVariant` schema structs (without fields — fields are
    /// stored at the `EmbeddedEnum` level).
    pub(super) fn expand_enum_variants(&self) -> Vec<TokenStream> {
        let toasty = &self.toasty;
        let embedded_enum = self.model.kind.as_embedded_enum_unwrap();

        embedded_enum
            .variants
            .iter()
            .map(|variant| {
                let variant_name = schema::expand_name(toasty, &variant.name);
                let discriminant_expr =
                    self.expand_discriminant_schema(&variant.attrs.discriminant);
                quote! {
                    #toasty::core::schema::app::EnumVariant {
                        name: #variant_name,
                        discriminant: #discriminant_expr,
                    }
                }
            })
            .collect()
    }

    /// Expands a `VariantValue` to a `Value` token for use in schema registration.
    fn expand_discriminant_schema(&self, value: &VariantValue) -> TokenStream {
        let toasty = &self.toasty;
        match value {
            VariantValue::Integer(n) => {
                quote! { #toasty::core::stmt::Value::I64(#n) }
            }
            VariantValue::String(s) => {
                quote! { #toasty::core::stmt::Value::String(#s.to_string()) }
            }
        }
    }

    /// Expands a discriminant value to a `Value::I64(n)` or `Value::String(s.into())` expression.
    fn expand_discriminant_value_expr(&self, value: &VariantValue) -> TokenStream {
        let toasty = &self.toasty;
        match value {
            VariantValue::Integer(n) => {
                quote! { #toasty::core::stmt::Expr::Value(#toasty::core::stmt::Value::I64(#n)) }
            }
            VariantValue::String(s) => {
                quote! { #toasty::core::stmt::Expr::Value(#toasty::core::stmt::Value::String(#s.into())) }
            }
        }
    }

    /// Generates the flat list of `Field` schema tokens for all variant fields,
    /// with each field tagged with its `VariantId`.
    pub(super) fn expand_enum_schema_fields(&self) -> Vec<TokenStream> {
        let toasty = &self.toasty;

        // Effective `#[column("name")]` override per shared group: declaring
        // the override on one sharing field suffices, so propagate it to every
        // member. Disagreeing overrides are rejected by
        // `expand_shared_column_checks`; here the first one wins.
        let mut shared_overrides: HashMap<String, &syn::LitStr> = HashMap::new();
        for field in &self.model.fields {
            let Some(ident) = &field.attrs.shared else {
                continue;
            };
            let Some(lit) = field.attrs.column.as_ref().and_then(|c| c.name.as_ref()) else {
                continue;
            };
            shared_overrides.entry(ident.to_string()).or_insert(lit);
        }

        self.model
            .fields
            .iter()
            .map(|field| {
                let index = util::int(field.id);
                let app_name = field.name.as_str();
                // A `#[column("name")]` on a variant field overrides its
                // database column name. For a `#[shared]` field, the override
                // declared by any member of the group applies to all of them.
                let storage_override = field
                    .attrs
                    .column
                    .as_ref()
                    .and_then(|column| column.name.as_ref())
                    .or_else(|| {
                        let ident = field.attrs.shared.as_ref()?.to_string();
                        shared_overrides.get(&ident).copied()
                    });
                let storage_name = match storage_override {
                    Some(name) => quote! { Some(#name.to_string()) },
                    None => quote! { None },
                };
                // `#[shared(<ident>)]` names the logical field this variant
                // field participates in. Fields declaring the same identifier
                // are coalesced into one shared column by the schema builder
                // (see `BuildMapping::map_field_primitive`).
                let shared = match &field.attrs.shared {
                    Some(ident) => {
                        let name = Name::from_ident(ident);
                        let name = schema::expand_name(toasty, &name);
                        quote! { Some(#name) }
                    }
                    None => quote! { None },
                };
                let ty = primitive_ty_unwrap(field);
                let storage_ty = match field
                    .attrs
                    .column
                    .as_ref()
                    .and_then(|column| column.ty.as_ref())
                {
                    Some(column_ty) => {
                        let expanded = column_ty.expand_with(toasty);
                        quote! { Some(#expanded) }
                    }
                    None => quote! { None },
                };
                let variant_index = field.variant.expect("enum field must have variant");
                let variant_idx = util::int(variant_index);
                quote! {
                    #toasty::core::schema::app::Field {
                        id: #toasty::core::schema::app::FieldId {
                            model: id,
                            index: #index,
                        },
                        name: #toasty::core::schema::app::FieldName {
                            app: Some(#app_name.to_string()),
                            storage: #storage_name,
                        },
                        ty: <#ty as #toasty::Field>::field_ty(#storage_ty),
                        nullable: <#ty as #toasty::Field>::NULLABLE,
                        primary_key: false,
                        auto: None,
                        versionable: false,
                        deferred: <#ty as #toasty::Field>::DEFERRED,
                        constraints: vec![],
                        variant: Some(#toasty::core::schema::app::VariantId {
                            model: id,
                            index: #variant_idx,
                        }),
                        shared: #shared,
                    }
                }
            })
            .collect()
    }

    /// Emits compile-time checks for variant fields that declare a shared
    /// logical field via `#[shared(<ident>)]`.
    ///
    /// Fields declaring the same identifier coalesce into one shared column,
    /// which can hold only one storage type. This emits the following
    /// obligations rather than deferring to a schema-build error:
    ///
    /// - **Same-variant collision:** two fields in the *same* variant sharing
    ///   one identifier is invalid — both are active for one discriminant, so
    ///   the column can't hold both. Emit a `compile_error!` pinned to the
    ///   second.
    /// - **Type agreement:** across variants, every sharing field must have the
    ///   same type. Emit a `SameColumnType` bound between each field's type and
    ///   the first in its group; unequal types fail to compile.
    /// - **Column-name agreement:** a `#[column("...")]` override declared by
    ///   one member of a group applies to the whole group, so two members
    ///   declaring *different* overrides is a contradiction.
    ///
    /// Duplicate columns *without* `#[shared]` are **not** checked here. The
    /// macro cannot tell a scalar from an embedded type, and for an embedded
    /// field `#[column("...")]` is a flattening prefix rather than a single
    /// column name — two variants may give embedded fields the same prefix and
    /// still flatten to disjoint columns. The schema builder checks the actual
    /// flattened leaf column names (`BuildMapping::map_field_primitive`).
    pub(super) fn expand_shared_column_checks(&self) -> TokenStream {
        let toasty = &self.toasty;

        // Group variant fields by their `#[shared]` identifier, preserving
        // declaration order within each group.
        let mut groups: Vec<(String, Vec<&crate::model::schema::Field>)> = Vec::new();
        for field in &self.model.fields {
            let Some(ident) = &field.attrs.shared else {
                continue;
            };
            let name = ident.to_string();

            match groups.iter_mut().find(|(existing, _)| *existing == name) {
                Some((_, fields)) => fields.push(field),
                None => groups.push((name, vec![field])),
            }
        }

        let mut checks = Vec::new();
        for (name, fields) in &groups {
            // Reject a second field in the same variant declaring this ident.
            let mut seen_variants = Vec::new();
            let mut same_variant = false;
            for field in fields {
                let variant = field
                    .variant
                    .expect("enum variant field must have a variant");
                if seen_variants.contains(&variant) {
                    let ident = &field.name.ident;
                    checks.push(
                        syn::Error::new_spanned(
                            ident,
                            format!(
                                "two fields in the same variant declare `#[shared({name})]`; \
                                 a column can be shared only across different variants"
                            ),
                        )
                        .to_compile_error(),
                    );
                    same_variant = true;
                } else {
                    seen_variants.push(variant);
                }
            }

            // A malformed group can't be meaningfully checked further; the
            // collision error above is the actionable one.
            if same_variant {
                continue;
            }

            // A `#[column("...")]` override on any member applies to the whole
            // group; disagreeing overrides are a contradiction.
            let mut column_override: Option<&syn::LitStr> = None;
            for field in fields {
                let Some(lit) = field.attrs.column.as_ref().and_then(|c| c.name.as_ref()) else {
                    continue;
                };
                match column_override {
                    None => column_override = Some(lit),
                    Some(first) if first.value() == lit.value() => {}
                    Some(first) => {
                        checks.push(
                            syn::Error::new_spanned(
                                lit,
                                format!(
                                    "fields sharing `{name}` declare conflicting column \
                                     names `{}` and `{}`; a shared field maps to one column",
                                    first.value(),
                                    lit.value(),
                                ),
                            )
                            .to_compile_error(),
                        );
                    }
                }
            }

            let first = primitive_ty_unwrap(fields[0]);
            for field in &fields[1..] {
                let ty = primitive_ty_unwrap(field);
                checks.push(quote_spanned! { ty.span()=>
                    const _: () = {
                        fn check<A, B>()
                        where
                            A: #toasty::shared_column::SameColumnType<B>,
                        {
                        }
                        let _ = check::<#ty, #first>;
                    };
                });
            }
        }

        quote! { #( #checks )* }
    }

    /// Generates the full `impl Load for Enum { ... }` block, adapting
    /// discriminant matching based on whether integer or string discriminants are used.
    pub(super) fn expand_enum_load_impl(&self) -> TokenStream {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        let ty_expr = self.expand_enum_primitive_ty();

        let unit_load_arms = self.expand_enum_load_arms(true);
        let data_load_arms = self.expand_enum_load_arms(false);

        // Generate discriminant-specific match tokens. The outer match uses
        // `ref` so `value` stays available for the error path. The inner
        // (record) match owns the taken element.
        let (ref_pattern, owned_pattern, match_expr) = if self.uses_string_discriminants() {
            (
                quote! { #toasty::core::stmt::Value::String(ref d) },
                quote! { #toasty::core::stmt::Value::String(d) },
                quote! { d.as_str() },
            )
        } else {
            (
                quote! { #toasty::core::stmt::Value::I64(ref d) },
                quote! { #toasty::core::stmt::Value::I64(d) },
                quote! { d },
            )
        };

        quote! {
            impl #toasty::Load for #model_ident {
                type Output = Self;

                fn ty() -> #toasty::core::stmt::Type {
                    #ty_expr
                }

                fn load(value: #toasty::core::stmt::Value) -> #toasty::Result<Self> {
                    match value {
                        #ref_pattern => match #match_expr {
                            #( #unit_load_arms )*
                            _ => Err(#toasty::Error::type_conversion(
                                value,
                                stringify!(#model_ident),
                            )),
                        },
                        #toasty::core::stmt::Value::Record(mut record) => match record[0].take() {
                            #owned_pattern => match #match_expr {
                                #( #data_load_arms )*
                                _ => Err(#toasty::Error::type_conversion(
                                    #owned_pattern,
                                    stringify!(#model_ident),
                                )),
                            },
                            other => Err(#toasty::Error::type_conversion(
                                other,
                                stringify!(#model_ident),
                            )),
                        },
                        value => Err(#toasty::Error::type_conversion(value, stringify!(#model_ident))),
                    }
                }

                fn reload(target: &mut Self, value: #toasty::core::stmt::Value) -> #toasty::Result<()> {
                    *target = Self::load(value)?;
                    Ok(())
                }
            }
        }
    }

    /// Generates match arms for Load. When `unit_only` is true, emits arms for
    /// unit variants; otherwise emits arms for data-carrying variants.
    fn expand_enum_load_arms(&self, unit_only: bool) -> Vec<TokenStream> {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        let embedded_enum = self.model.kind.as_embedded_enum_unwrap();

        embedded_enum
            .variants
            .iter()
            .enumerate()
            .filter(|(variant_index, _)| {
                self.variant_fields(*variant_index).is_empty() == unit_only
            })
            .map(|(variant_index, variant)| {
                let ident = &variant.ident;
                let pattern = expand_discriminant_match_pattern(&variant.attrs.discriminant);

                let construction = if unit_only {
                    quote! { #model_ident::#ident }
                } else {
                    let fields = self.variant_fields(variant_index);
                    let field_loads: Vec<_> = fields
                        .iter()
                        .enumerate()
                        .map(|(i, field)| {
                            let field_ident = &field.name.ident;
                            let ty = primitive_ty_unwrap(field);
                            let record_pos = util::int(i + 1);
                            let load = quote! {
                                <#ty as #toasty::Load>::load(record[#record_pos].take())?
                            };
                            if variant.fields_named {
                                quote! { #field_ident: #load, }
                            } else {
                                quote! { #load, }
                            }
                        })
                        .collect();

                    if variant.fields_named {
                        quote! { #model_ident::#ident { #( #field_loads )* } }
                    } else {
                        quote! { #model_ident::#ident( #( #field_loads )* ) }
                    }
                };

                quote! { #pattern => Ok(#construction), }
            })
            .collect()
    }

    /// Generates match arms for `IntoExpr::into_expr` and `IntoExpr::by_ref`.
    /// Unit variants emit `Value::I64(discriminant)`. Data variants emit
    /// `Value::Record([I64(disc), field_exprs...])`. The same arms work for both
    /// methods: for `by_ref(&self)` match ergonomics bind field names as `&T`,
    /// and `IntoExpr` is implemented for both `T` and `&T`.
    pub(super) fn expand_enum_into_expr_arms(&self) -> Vec<TokenStream> {
        let toasty = &self.toasty;
        let model_ident = &self.model.ident;
        let embedded_enum = self.model.kind.as_embedded_enum_unwrap();

        embedded_enum
            .variants
            .iter()
            .enumerate()
            .map(|(variant_index, variant)| {
                let ident = &variant.ident;
                let fields = self.variant_fields(variant_index);

                let discriminant_expr =
                    self.expand_discriminant_value_expr(&variant.attrs.discriminant);

                if fields.is_empty() {
                    // In a mixed enum (has_data_variants), the model value is always a
                    // Record so that `project([0])` uniformly extracts the discriminant
                    // for the `model_to_table` mapping.  Unit-only enums keep the plain
                    // I64 form (cheaper and the load path expects it).
                    if self.expand_enum_has_data_variants() {
                        quote! {
                            #model_ident::#ident => #toasty::stmt::Expr::from_untyped(
                                #toasty::core::stmt::Expr::record([#discriminant_expr])
                            ),
                        }
                    } else {
                        quote! {
                            #model_ident::#ident => #toasty::stmt::Expr::from_untyped(
                                #discriminant_expr
                            ),
                        }
                    }
                } else {
                    let field_idents: Vec<_> = fields.iter().map(|f| &f.name.ident).collect();

                    let pattern = if variant.fields_named {
                        quote! { #model_ident::#ident { #( #field_idents ),* } }
                    } else {
                        quote! { #model_ident::#ident( #( #field_idents ),* ) }
                    };

                    let field_exprs = fields.iter().map(|field| {
                        let field_ident = &field.name.ident;
                        let ty = primitive_ty_unwrap(field);
                        quote!(#toasty::into_untyped_expr::<FieldExprTarget<#ty>, _>(#field_ident))
                    });

                    quote! {
                        #pattern =>
                            #toasty::stmt::Expr::from_untyped(
                                #toasty::core::stmt::Expr::record([
                                    #discriminant_expr,
                                    #( #field_exprs ),*
                                ])
                            ),
                    }
                }
            })
            .collect()
    }

    /// Generates the `Field::ty()` return expression. Unit-only enums map to
    /// `Type::I64` or `Type::String`; enums with at least one data variant map to `Type::Model`.
    pub(super) fn expand_enum_primitive_ty(&self) -> TokenStream {
        let toasty = &self.toasty;
        if self.expand_enum_has_data_variants() {
            quote! { #toasty::core::stmt::Type::Model(<Self as #toasty::Embed>::id()) }
        } else if self.uses_string_discriminants() {
            quote! { #toasty::core::stmt::Type::String }
        } else {
            quote! { #toasty::core::stmt::Type::I64 }
        }
    }

    /// Generates the `storage_ty` token for the discriminant `FieldPrimitive`.
    ///
    /// - Native enum: `Some(db::Type::Enum(TypeEnum { ... }))`
    /// - Plain string (`#[column(type = text)]`): `Some(db::Type::Text)`
    /// - Explicit integer type: the requested integer storage type
    /// - Default integer discriminants: `None`
    pub(super) fn expand_enum_storage_ty(&self) -> TokenStream {
        let toasty = &self.toasty;
        let embedded_enum = self.model.kind.as_embedded_enum_unwrap();

        match &embedded_enum.storage_strategy {
            Some(EnumStorageStrategy::NativeEnum(custom_name)) => {
                // Determine the type name: custom name or default snake_case of enum ident.
                let type_name = match custom_name {
                    Some(name) => name.clone(),
                    None => {
                        use heck::ToSnakeCase;
                        self.model.ident.to_string().to_snake_case()
                    }
                };

                // Collect variant names in declaration order.
                let variant_names: Vec<&str> = embedded_enum
                    .variants
                    .iter()
                    .map(|v| match &v.attrs.discriminant {
                        VariantValue::String(s) => s.as_str(),
                        _ => unreachable!("native enum requires string discriminants"),
                    })
                    .collect();

                quote! {
                    ::std::option::Option::Some(
                        #toasty::core::schema::db::Type::Enum(
                            #toasty::core::schema::db::TypeEnum {
                                name: ::std::option::Option::Some(#type_name.to_string()),
                                variants: vec![
                                    #( #toasty::core::schema::db::EnumVariant {
                                        name: #variant_names.to_string(),
                                    } ),*
                                ],
                            }
                        )
                    )
                }
            }
            Some(EnumStorageStrategy::PlainString(col_ty)) => {
                let ty_tokens = col_ty.expand_with(toasty);
                quote! { ::std::option::Option::Some(#ty_tokens) }
            }
            Some(EnumStorageStrategy::Integer(col_ty)) => {
                let ty_tokens = col_ty.expand_with(toasty);
                quote! { ::std::option::Option::Some(#ty_tokens) }
            }
            None => {
                // Integer discriminants: no storage type hint.
                quote! { ::std::option::Option::None }
            }
        }
    }
}

fn primitive_ty_unwrap(field: &crate::model::schema::Field) -> &syn::Type {
    match &field.ty {
        FieldTy::Primitive(ty) => ty,
        _ => panic!("expected primitive field type for enum variant field"),
    }
}

/// Generates the match pattern token for a discriminant value:
/// a string literal for `VariantValue::String`, an integer literal for `VariantValue::Integer`.
fn expand_discriminant_match_pattern(value: &VariantValue) -> TokenStream {
    match value {
        VariantValue::String(s) => quote! { #s },
        VariantValue::Integer(n) => quote! { #n },
    }
}
