use super::AutoStrategy;

use super::{BelongsTo, Column, ErrorSet, HasMany, HasOne, ItemParent, ItemParentAttr, Name};

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

    /// If this field belongs to an enum variant, the variant's index within
    /// the enum. `None` for fields on root models and embedded structs.
    pub(crate) variant: Option<usize>,

    /// `#[item_parent]` marker, if present. The parent type will be
    /// extracted from the field's `Deferred<T>` type in a later step.
    pub(crate) item_parent: Option<ItemParentAttr>,

    /// Parent type extracted from the field's `Deferred<T>` type when
    /// `#[item_parent]` is set. `None` otherwise.
    pub(crate) item_parent_target: Option<syn::Type>,
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

    /// True if the field tracks an OCC version counter
    pub(crate) versionable: bool,
}

#[derive(Debug)]
pub(crate) enum FieldTy {
    Primitive(syn::Type),
    BelongsTo(BelongsTo),
    HasMany(HasMany),
    HasOne(HasOne),
    /// Synthesised by the model post-pass when a field carries
    /// `#[item_parent]`. Distinct from `BelongsTo` because navigation
    /// lowers to a partition-scoped query rather than a value-equality
    /// join (design R2.9). B4.7 introduced this variant; B4.8/B4.9 wire
    /// the relation method and HasMany pairing.
    ItemParent(ItemParent),
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
            versionable: false,
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
                if !matches!(attr.meta, syn::Meta::Path(_)) {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "field-level `#[index]` does not take arguments; \
                         for a composite index spanning multiple fields, use a \
                         struct-level `#[index(field1, field2, ...)]` attribute on the model",
                    ));
                } else if field_attr.index {
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
            } else if attr.path().is_ident("serialize") {
                // The `#[serialize(json)]` attribute has been replaced by the
                // `toasty::Json<T>` field wrapper. The wrapper handles the
                // same encoding through trait dispatch, composes cleanly with
                // `Option<T>` and `Deferred<T>`, and works in expressions
                // (e.g. `.eq(Json("hello"))`).
                errs.push(syn::Error::new_spanned(
                    attr,
                    "the `#[serialize(json)]` attribute has been removed; \
                     wrap the field type in `toasty::Json<T>` instead \
                     (e.g. `tags: toasty::Json<Vec<String>>`)",
                ));
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        Ok(field_attr)
    }
}

impl Field {
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
        let mut item_parent: Option<ItemParentAttr> = None;

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
            } else if attr.path().is_ident("item_parent") {
                if item_parent.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[item_parent] attribute on field",
                    ));
                } else {
                    match ItemParentAttr::from_ast(attr) {
                        Ok(parsed) => item_parent = Some(parsed),
                        Err(e) => errs.push(e),
                    }
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

        // Validate `#[item_parent]` field type and extract `T` from `Deferred<T>`.
        let mut item_parent_target: Option<syn::Type> = None;
        if let Some(parent_attr) = &item_parent {
            // Reject `#[item_parent]` combined with a relation attribute
            // (`#[belongs_to]`, `#[has_many]`, `#[has_one]`); they're mutually
            // exclusive — `#[item_parent]` is the only relation-like marker
            // allowed for item-collection children.
            if ty.is_some() {
                errs.push(syn::Error::new(
                    parent_attr.span,
                    "field has both `#[item_parent]` and a relation attribute (`#[belongs_to]`, `#[has_many]`, or `#[has_one]`); use only `#[item_parent]` for item-collection children",
                ));
            }

            match super::item_parent::extract_deferred_inner(&name.ident, &field.ty) {
                Ok(parent_ty) => item_parent_target = Some(parent_ty),
                Err(e) => errs.push(e),
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
            // `Field::from_ast` only produces `Primitive`/`BelongsTo`/`HasMany`/
            // `HasOne` — `ItemParent` is synthesised by the model post-pass in
            // `Model::from_ast` after this method runs, so it cannot appear here.
            FieldTy::ItemParent(_) => unreachable!(
                "ItemParent is synthesised after Field::from_ast; \
                 see Model::from_ast post-pass"
            ),
        }

        Ok(Self {
            id,
            attrs,
            name,
            ty,
            set_ident,
            variant: None,
            item_parent,
            item_parent_target,
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
