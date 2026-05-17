use super::Type;

/// The type of a document-stored value: a named, ordered sequence of fields.
///
/// A document type carries the field *names* alongside their types, unlike
/// [`Type::Record`](super::Type::Record), which is positional and nameless.
/// The names are what let the query engine convert between the positional
/// [`Value::Record`](super::Value::Record) the rest of the pipeline uses and
/// the named [`Value::Object`](super::Value::Object) handed to drivers that
/// store the field as a document (JSON, BSON, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeDocument {
    /// The document's fields, in declaration order.
    pub fields: Vec<DocumentField>,
}

/// A single named field within a [`TypeDocument`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentField {
    /// The field name, used as the document key.
    pub name: String,

    /// The field's type.
    pub ty: Type,
}

impl TypeDocument {
    /// Creates a document type from its fields.
    pub fn new(fields: Vec<DocumentField>) -> Self {
        Self { fields }
    }

    /// Returns the number of fields in the document.
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns `true` if the document has no fields.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}
