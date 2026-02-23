use crate::schema::AutoStrategy;

use super::{BelongsTo, Column, ErrorSet, HasMany, HasOne, Name};

use syn::spanned::Spanned;

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

    /// Identifier for the `with_field` builder method on update builder
    pub(crate) with_ident: syn::Ident,
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
}

#[derive(Debug)]
pub(crate) enum FieldTy {
    Primitive(syn::Type),
    BelongsTo(BelongsTo),
    HasMany(HasMany),
    HasOne(HasOne),
}

impl FieldTy {
    pub(crate) fn is_relation(&self) -> bool {
        matches!(
            self,
            Self::BelongsTo(..) | Self::HasMany(..) | Self::HasOne(..)
        )
    }
}

impl Field {
    pub(super) fn from_ast(
        field: &syn::Field,
        model_ident: &syn::Ident,
        id: usize,
        names: &[syn::Ident],
    ) -> syn::Result<Self> {
        let Some(ident) = &field.ident else {
            return Err(syn::Error::new_spanned(field, "model fields must be named"));
        };

        let name = Name::from_ident(ident);
        let set_ident = syn::Ident::new(&format!("set_{}", name.ident), ident.span());
        let with_ident = syn::Ident::new(&format!("with_{}", name.ident), ident.span());

        let mut errs = ErrorSet::new();
        let mut attrs = FieldAttr {
            key: None,
            unique: false,
            auto: None,
            index: false,
            column: None,
            default_expr: None,
            update_expr: None,
        };

        let mut ty = None;

        for attr in &field.attrs {
            if attr.path().is_ident("key") {
                if attrs.key.is_some() {
                    errs.push(syn::Error::new_spanned(attr, "duplicate #[key] attribute"));
                } else {
                    attrs.key = Some(attr.clone());
                }
            } else if attr.path().is_ident("auto") {
                if attrs.auto.is_some() {
                    errs.push(syn::Error::new_spanned(attr, "duplicate #[auto] attribute"));
                } else {
                    attrs.auto = Some(AutoStrategy::from_ast(attr)?);
                }
            } else if attr.path().is_ident("unique") {
                if attrs.unique {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[unique] attribute",
                    ));
                } else {
                    attrs.unique = true;
                }
            } else if attr.path().is_ident("index") {
                if attrs.index {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[index] attribute",
                    ));
                } else {
                    attrs.index = true;
                }
            } else if attr.path().is_ident("belongs_to") {
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
                        ident,
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
                    ty = Some(FieldTy::HasOne(HasOne::from_ast(&field.ty, field.span())?));
                }
            } else if attr.path().is_ident("column") {
                if attrs.column.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[column] attribute",
                    ));
                } else {
                    attrs.column = Some(Column::from_ast(attr)?);
                }
            } else if attr.path().is_ident("default") {
                if attrs.default_expr.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[default] attribute",
                    ));
                } else {
                    attrs.default_expr = Some(attr.parse_args()?);
                }
            } else if attr.path().is_ident("update") {
                if attrs.update_expr.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[update] attribute",
                    ));
                } else {
                    attrs.update_expr = Some(attr.parse_args()?);
                }
            } else if attr.path().is_ident("toasty") {
                // todo
            }
        }

        // Expand #[auto] on timestamp fields:
        //   created_at → #[default(jiff::Timestamp::now())]
        //   updated_at → #[update(jiff::Timestamp::now())]
        if matches!(&attrs.auto, Some(AutoStrategy::Unspecified)) {
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
            with_ident,
        })
    }
}

fn rewrite_self(ty: &mut syn::Type, model: &syn::Ident) {
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
