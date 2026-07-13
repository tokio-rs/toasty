//! Doc comments for the derive-generated public methods.
//!
//! Emitting real docs (rather than `#[allow(missing_docs)]`) keeps models
//! usable from crates that `#![deny(missing_docs)]`. Plain code spans are used
//! instead of intra-doc links so no `broken_intra_doc_links` warnings leak
//! into the user's crate.

use super::{Expand, Filter};

impl Expand<'_> {
    pub(super) fn doc_create(&self) -> String {
        format!(
            "Returns a builder that inserts a new `{}` record.\n\n\
             Typically constructed through the `toasty::create!` macro.",
            self.model.ident
        )
    }

    pub(super) fn doc_create_many(&self) -> String {
        format!(
            "Returns a builder that inserts multiple `{}` records at once.",
            self.model.ident
        )
    }

    pub(super) fn doc_update(&self) -> String {
        format!(
            "Returns a builder that updates this `{}` record in place.",
            self.model.ident
        )
    }

    pub(super) fn doc_all(&self) -> String {
        format!(
            "Returns a query that selects every `{}` record.",
            self.model.ident
        )
    }

    pub(super) fn doc_filter(&self) -> String {
        format!(
            "Returns a query selecting the `{}` records matching `expr`.",
            self.model.ident
        )
    }

    pub(super) fn doc_delete(&self) -> String {
        format!(
            "Consumes this `{}` and returns a statement that deletes its record.",
            self.model.ident
        )
    }

    pub(super) fn doc_fields(&self) -> String {
        format!(
            "Returns a handle used to reference `{}`'s fields when building query expressions.",
            self.model.ident
        )
    }

    pub(super) fn doc_filter_get(&self, filter: &Filter) -> String {
        format!(
            "Fetches the single `{}` where {}, erroring if none exists.",
            self.model.ident,
            self.filter_fields_phrase(filter)
        )
    }

    pub(super) fn doc_filter_update(&self, filter: &Filter) -> String {
        format!(
            "Returns a builder that updates the `{}` where {}.",
            self.model.ident,
            self.filter_fields_phrase(filter)
        )
    }

    pub(super) fn doc_filter_delete(&self, filter: &Filter) -> String {
        format!(
            "Deletes the `{}` where {}.",
            self.model.ident,
            self.filter_fields_phrase(filter)
        )
    }

    pub(super) fn doc_filter_query(&self, filter: &Filter) -> String {
        format!(
            "Returns a query selecting the `{}` records where {}.",
            self.model.ident,
            self.filter_fields_phrase(filter)
        )
    }

    pub(super) fn doc_belongs_to(&self, field_ident: &syn::Ident) -> String {
        format!(
            "Returns a query for the `{field_ident}` record this `{}` belongs to.",
            self.model.ident
        )
    }

    pub(super) fn doc_has_many(&self, field_ident: &syn::Ident) -> String {
        format!(
            "Returns a query over the `{field_ident}` associated with this `{}`.",
            self.model.ident
        )
    }

    pub(super) fn doc_has_many_via(&self, field_ident: &syn::Ident) -> String {
        format!(
            "Returns a query over the `{field_ident}` reached from this `{}` through its relations.",
            self.model.ident
        )
    }

    pub(super) fn doc_has_one(&self, field_ident: &syn::Ident) -> String {
        format!(
            "Returns a query for the `{field_ident}` associated with this `{}`.",
            self.model.ident
        )
    }

    /// A human-readable phrase naming the fields a filter matches on — e.g.
    /// `` `email` matches `` or `` `a` and `b` match ``.
    fn filter_fields_phrase(&self, filter: &Filter) -> String {
        let names: Vec<String> = filter
            .fields
            .iter()
            .map(|index| format!("`{}`", self.model.fields[*index].name.as_str()))
            .collect();

        if names.len() == 1 {
            format!("{} matches", names[0])
        } else {
            format!("{} match", names.join(" and "))
        }
    }
}
