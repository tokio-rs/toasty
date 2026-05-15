use super::AutoStrategy;

use super::{BelongsTo, Column, ErrorSet, HasMany, HasOne, Name};

use syn::spanned::Spanned;

/// Codegen-level representation of a serialization format.
#[derive(Debug, Clone)]
pub(crate) enum SerializeFormat {
    Json,
}

/// Parsed `#[serialize(...)]` attribute data.
#[derive(Debug, Clone)]
pub(crate) struct SerializeAttr {
    pub(crate) format: SerializeFormat,
    pub(crate) nullable: bool,
}

/// Parsed `#[document]` / `#[document(text)]` attribute data.
///
/// `#[document]` forces a field into document storage. The `text` modifier
/// (`#[document(text)]`) selects PostgreSQL's text `json` over `jsonb`; it is
/// parsed here but rejected during validation until the text encoding path is
/// wired up.
#[derive(Debug, Clone)]
pub(crate) struct DocumentAttr {
    /// True if `#[document(text)]` was written (the `binary: false` encoding).
    pub(crate) text: bool,

    /// The originating attribute, retained for span-accurate diagnostics.
    pub(crate) attr: syn::Attribute,
}

#[derive(Debug)]
pub(crate) struct Field {
    /// Index of field in the containing model
    pub(crate) id: usize,

    /// Field attributes
    pub(crate) attrs: FieldAttr,

    /// Field name
    pub(crate) name: Name,

    /// Field type
    pub(crate) ty: FieldTy,

    /// Identifier for setter method on update builder
    pub(crate) set_ident: syn::Ident,

    /// If this field belongs to an enum variant, the variant's index within
    /// the enum. `None` for fields on root models and embedded structs.
    pub(crate) variant: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct FieldAttr {
    /// True if the field is annotated with `#[key]`
    pub(crate) key: Option<syn::Attribute>,

    /// True if the field is annotated with `#[unique]`
    pub(crate) unique: bool,

    /// Specifies if and how Toasty should automatically set values of newly created rows
    pub(crate) auto: Option<AutoStrategy>,

    /// True if the field is indexed
    pub(crate) index: bool,

    /// Optional database column name and / or type
    pub(crate) column: Option<Column>,

    /// Expression to use as default value on create: `#[default(<expr>)]`
    pub(crate) default_expr: Option<syn::Expr>,

    /// Expression to apply on create and update: `#[update(<expr>)]`
    pub(crate) update_expr: Option<syn::Expr>,

    /// Serialization info for the field: `#[serialize(json)]` or `#[serialize(json, nullable)]`
    pub(crate) serialize: Option<SerializeAttr>,

    /// Document-storage info for the field: `#[document]` or `#[document(text)]`
    pub(crate) document: Option<DocumentAttr>,

    /// True if the field tracks an OCC version counter
    pub(crate) versionable: bool,

    /// True if the field is annotated with `#[deferred]`
    pub(crate) deferred: bool,
}

#[derive(Debug)]
pub(crate) enum FieldTy {
    Primitive(syn::Type),
    BelongsTo(BelongsTo),
    HasMany(HasMany),
    HasOne(HasOne),
}

impl FieldAttr {
    pub(crate) fn is_indexed(&self) -> bool {
        self.unique || self.index
    }

    /// Parse `FieldAttr`-related attributes from an attribute list.
    ///
    /// Handles `#[key]`, `#[auto]`, `#[unique]`, `#[index]`, `#[column]`,
    /// `#[default]`, and `#[update]`. Other attributes are silently skipped.
    pub(crate) fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut errs = ErrorSet::new();
        let mut field_attr = FieldAttr {
            key: None,
            unique: false,
            auto: None,
            index: false,
            column: None,
            default_expr: None,
            update_expr: None,
            serialize: None,
            document: None,
            versionable: false,
            deferred: false,
        };

        for attr in attrs {
            if attr.path().is_ident("key") {
                if field_attr.key.is_some() {
                    errs.push(syn::Error::new_spanned(attr, "duplicate #[key] attribute"));
                } else {
                    field_attr.key = Some(attr.clone());
                }
            } else if attr.path().is_ident("auto") {
                if field_attr.auto.is_some() {
                    errs.push(syn::Error::new_spanned(attr, "duplicate #[auto] attribute"));
                } else {
                    match AutoStrategy::from_ast(attr) {
                        Ok(strategy) => field_attr.auto = Some(strategy),
                        Err(e) => errs.push(e),
                    }
                }
            } else if attr.path().is_ident("unique") {
                if field_attr.unique {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[unique] attribute",
                    ));
                } else {
                    field_attr.unique = true;
                }
            } else if attr.path().is_ident("index") {
                if field_attr.index {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[index] attribute",
                    ));
                } else {
                    field_attr.index = true;
                }
            } else if attr.path().is_ident("column") {
                if field_attr.column.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[column] attribute",
                    ));
                } else {
                    match Column::from_ast(attr) {
                        Ok(col) => field_attr.column = Some(col),
                        Err(e) => errs.push(e),
                    }
                }
            } else if attr.path().is_ident("default") {
                if field_attr.default_expr.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[default] attribute",
                    ));
                } else {
                    match attr.parse_args() {
                        Ok(expr) => field_attr.default_expr = Some(expr),
                        Err(e) => errs.push(e),
                    }
                }
            } else if attr.path().is_ident("update") {
                if field_attr.update_expr.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[update] attribute",
                    ));
                } else {
                    match attr.parse_args() {
                        Ok(expr) => field_attr.update_expr = Some(expr),
                        Err(e) => errs.push(e),
                    }
                }
            } else if attr.path().is_ident("version") {
                if field_attr.versionable {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[version] attribute",
                    ));
                } else {
                    field_attr.versionable = true;
                }
            } else if attr.path().is_ident("deferred") {
                if field_attr.deferred {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[deferred] attribute",
                    ));
                } else {
                    field_attr.deferred = true;
                }
            } else if attr.path().is_ident("serialize") {
                if field_attr.serialize.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[serialize] attribute",
                    ));
                } else {
                    match attr.parse_args_with(
                        syn::punctuated::Punctuated::<syn::Ident, syn::Token![,]>::parse_terminated,
                    ) {
                        Ok(args) => {
                            let mut format = None;
                            let mut nullable = false;

                            for arg in &args {
                                if arg == "json" {
                                    if format.is_some() {
                                        errs.push(syn::Error::new_spanned(
                                            arg,
                                            "duplicate format specifier",
                                        ));
                                    } else {
                                        format = Some(SerializeFormat::Json);
                                    }
                                } else if arg == "nullable" {
                                    nullable = true;
                                } else {
                                    errs.push(syn::Error::new_spanned(
                                        arg,
                                        "unsupported serialize argument; expected `json` or `nullable`",
                                    ));
                                }
                            }

                            match format {
                                Some(format) => {
                                    field_attr.serialize = Some(SerializeAttr { format, nullable });
                                }
                                None => {
                                    errs.push(syn::Error::new_spanned(
                                        attr,
                                        "missing serialization format; expected `json`",
                                    ));
                                }
                            }
                        }
                        Err(e) => errs.push(e),
                    }
                }
            } else if attr.path().is_ident("document") {
                if field_attr.document.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[document] attribute",
                    ));
                } else {
                    let mut text = false;

                    match &attr.meta {
                        // Bare `#[document]`.
                        syn::Meta::Path(_) => {}
                        // `#[document(text)]` and friends.
                        syn::Meta::List(_) => {
                            match attr.parse_args_with(
                                syn::punctuated::Punctuated::<syn::Ident, syn::Token![,]>::parse_terminated,
                            ) {
                                Ok(args) => {
                                    for arg in &args {
                                        if arg == "text" {
                                            text = true;
                                        } else {
                                            errs.push(syn::Error::new_spanned(
                                                arg,
                                                "unsupported document argument; expected `text`",
                                            ));
                                        }
                                    }
                                }
                                Err(e) => errs.push(e),
                            }
                        }
                        syn::Meta::NameValue(_) => {
                            errs.push(syn::Error::new_spanned(
                                attr,
                                "#[document] does not take a value; use `#[document]` or `#[document(text)]`",
                            ));
                        }
                    }

                    field_attr.document = Some(DocumentAttr {
                        text,
                        attr: attr.clone(),
                    });
                }
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        Ok(field_attr)
    }
}

impl Field {
    /// The toasty schema trait this field's primitive codegen resolves
    /// through — `Document` for `#[document]` fields, `Field` otherwise.
    ///
    /// The two traits expose the same shape (`ExprTarget`, `NULLABLE`,
    /// `field_ty`, `register`), so callers can splice this ident into a
    /// `<#ty as #toasty::#trait_ident>::…` path uniformly.
    pub(crate) fn trait_ident(&self) -> proc_macro2::TokenStream {
        if self.attrs.document.is_some() {
            quote::quote!(Document)
        } else {
            quote::quote!(Field)
        }
    }

    pub(super) fn from_ast(
        field: &syn::Field,
        model_ident: &syn::Ident,
        id: usize,
        index: usize,
        names: &[syn::Ident],
    ) -> syn::Result<Self> {
        let (name, span) = match &field.ident {
            Some(ident) => (Name::from_ident(ident), ident.span()),
            None => {
                let span = field.ty.span();
                let ident = syn::Ident::new(&format!("_{index}"), span);
                let name = Name::from_ident(&ident);
                (name, span)
            }
        };

        let set_ident = syn::Ident::new(&name.with_prefix("set"), span);

        let mut attrs = FieldAttr::from_attrs(&field.attrs)?;

        let mut errs = ErrorSet::new();
        let mut ty = None;

        for attr in &field.attrs {
            if attr.path().is_ident("belongs_to") {
                if ty.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "field has more than one relation attribute",
                    ));
                } else {
                    ty = Some(FieldTy::BelongsTo(BelongsTo::from_ast(
                        attr, &field.ty, names,
                    )?));
                }
            } else if attr.path().is_ident("has_many") {
                if ty.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "field has more than one relation attribute",
                    ));
                } else {
                    ty = Some(FieldTy::HasMany(HasMany::from_ast(
                        attr,
                        field.ident.as_ref().unwrap(),
                        &field.ty,
                        field.span(),
                    )?));
                }
            } else if attr.path().is_ident("has_one") {
                if ty.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "field has more than one relation attribute",
                    ));
                } else {
                    ty = Some(FieldTy::HasOne(HasOne::from_ast(
                        attr,
                        &field.ty,
                        field.span(),
                    )?));
                }
            }
        }

        // Expand #[auto] on timestamp fields:
        //   created_at → #[default(jiff::Timestamp::now())]
        //   updated_at → #[update(jiff::Timestamp::now())]
        if matches!(&attrs.auto, Some(AutoStrategy::Unspecified))
            && let Some(ident) = &field.ident
        {
            let field_name = ident.to_string();
            let now_expr: syn::Expr = syn::parse_quote!(jiff::Timestamp::now());

            if field_name == "created_at" {
                attrs.auto = None;
                attrs.default_expr = Some(now_expr);
            } else if field_name == "updated_at" {
                attrs.auto = None;
                attrs.update_expr = Some(now_expr);
            }
        }

        if ty.is_some() && attrs.column.is_some() {
            errs.push(syn::Error::new_spanned(
                field,
                "relation fields cannot have a database type",
            ));
        }

        if ty.is_some() && attrs.default_expr.is_some() {
            errs.push(syn::Error::new_spanned(
                field,
                "#[default] cannot be used on relation fields",
            ));
        }

        if ty.is_some() && attrs.update_expr.is_some() {
            errs.push(syn::Error::new_spanned(
                field,
                "#[update] cannot be used on relation fields",
            ));
        }

        if ty.is_some() && attrs.serialize.is_some() {
            errs.push(syn::Error::new_spanned(
                field,
                "#[serialize] cannot be used on relation fields",
            ));
        }

        if attrs.auto.is_some() && attrs.default_expr.is_some() {
            errs.push(syn::Error::new_spanned(
                field,
                "#[default] and #[auto] cannot be combined on the same field",
            ));
        }

        if attrs.auto.is_some() && attrs.update_expr.is_some() {
            errs.push(syn::Error::new_spanned(
                field,
                "#[update] and #[auto] cannot be combined on the same field",
            ));
        }

        if attrs.versionable && attrs.key.is_some() {
            errs.push(syn::Error::new_spanned(
                field,
                "#[version] cannot be combined with #[key]",
            ));
        }

        if attrs.versionable && attrs.auto.is_some() {
            errs.push(syn::Error::new_spanned(
                field,
                "#[version] cannot be combined with #[auto]",
            ));
        }

        if attrs.versionable {
            let is_u64 = matches!(&field.ty, syn::Type::Path(p) if p.path.is_ident("u64"));
            if !is_u64 {
                errs.push(syn::Error::new_spanned(
                    &field.ty,
                    "#[version] can only be applied to a u64 field",
                ));
            }
        }

        if attrs.deferred {
            if ty.is_some() {
                errs.push(syn::Error::new_spanned(
                    field,
                    "#[deferred] cannot be combined with relation attributes",
                ));
            }

            if attrs.versionable {
                errs.push(syn::Error::new_spanned(
                    field,
                    "#[deferred] cannot be combined with #[version]",
                ));
            }

            if attrs.key.is_some() {
                errs.push(syn::Error::new_spanned(
                    field,
                    "#[deferred] cannot be combined with #[key]",
                ));
            }
        }

        if let Some(doc) = &attrs.document {
            // `#[document(text)]` is parsed but the text-encoding path is not
            // yet implemented.
            if doc.text {
                errs.push(syn::Error::new_spanned(
                    &doc.attr,
                    "#[document(text)] is not yet supported",
                ));
            }

            if ty.is_some() {
                errs.push(syn::Error::new_spanned(
                    field,
                    "#[document] cannot be combined with relation attributes",
                ));
            }

            if attrs.serialize.is_some() {
                errs.push(syn::Error::new_spanned(
                    field,
                    "#[document] cannot be combined with #[serialize]",
                ));
            }

            if attrs.key.is_some() {
                errs.push(syn::Error::new_spanned(
                    field,
                    "#[document] cannot be combined with #[key]",
                ));
            }

            if attrs.versionable {
                errs.push(syn::Error::new_spanned(
                    field,
                    "#[document] cannot be combined with #[version]",
                ));
            }

            if attrs.deferred {
                errs.push(syn::Error::new_spanned(
                    field,
                    "#[document] cannot be combined with #[deferred]",
                ));
            }

            if attrs.auto.is_some() {
                errs.push(syn::Error::new_spanned(
                    field,
                    "#[document] cannot be combined with #[auto]",
                ));
            }

            if attrs.is_indexed() {
                errs.push(syn::Error::new_spanned(
                    field,
                    "#[index] / #[unique] on a #[document] field is not yet supported",
                ));
            }

            if attrs.column.is_some() {
                errs.push(syn::Error::new_spanned(
                    field,
                    "#[column] on a #[document] field is not yet supported",
                ));
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        let mut ty = ty.unwrap_or_else(|| FieldTy::Primitive(field.ty.clone()));

        match &mut ty {
            FieldTy::BelongsTo(rel) => {
                rewrite_self(&mut rel.ty, model_ident);
            }
            FieldTy::HasMany(rel) => {
                rewrite_self(&mut rel.ty, model_ident);
            }
            FieldTy::HasOne(rel) => {
                rewrite_self(&mut rel.ty, model_ident);
            }
            FieldTy::Primitive(ty) => {
                rewrite_self(ty, model_ident);
            }
        }

        Ok(Self {
            id,
            attrs,
            name,
            ty,
            set_ident,
            variant: None,
        })
    }
}

pub(crate) fn rewrite_self(ty: &mut syn::Type, model: &syn::Ident) {
    use syn::visit_mut::VisitMut;

    struct RewriteSelf<'a>(&'a syn::Ident);

    impl VisitMut for RewriteSelf<'_> {
        fn visit_path_mut(&mut self, path: &mut syn::Path) {
            syn::visit_mut::visit_path_mut(self, path);

            if path.is_ident("Self") {
                // print!("SELF; ident={:#?}", self.0);
                path.segments[0].ident = self.0.clone();
            }
        }
    }

    RewriteSelf(model).visit_type_mut(ty);
}
