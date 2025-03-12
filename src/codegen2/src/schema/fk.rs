#[derive(Debug)]
pub(crate) struct ForeignKeyField {
    /// The field on the source struct
    pub(crate) source: usize,

    /// The identifier on the target struct being referenced
    pub(crate) target: syn::Ident,
}
