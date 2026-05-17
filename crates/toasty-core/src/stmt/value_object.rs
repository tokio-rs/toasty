use super::Value;

/// An ordered sequence of named [`Value`]s representing a document.
///
/// `ValueObject` is the named counterpart to [`ValueRecord`](super::ValueRecord):
/// where a record is positional, an object carries a key for each field. The
/// query engine builds a `ValueObject` from a positional record (using the
/// field names from [`TypeDocument`](super::TypeDocument)) just before handing
/// a document-stored value to a driver, and converts back the other way when
/// decoding driver results. Drivers serialize a `ValueObject` structurally —
/// to a JSON object, a BSON sub-document, a DynamoDB map — without needing the
/// schema.
///
/// Entries are kept in insertion order. Keys are not deduplicated; the engine
/// always builds objects from a schema, so keys are unique by construction.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ValueObject {
    /// The named field values, in insertion order.
    pub entries: Vec<(String, Value)>,
}

impl ValueObject {
    /// Creates a `ValueObject` from a vector of `(key, value)` pairs.
    pub fn from_vec(entries: Vec<(String, Value)>) -> Self {
        Self { entries }
    }

    /// Returns the number of entries in the object.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the object has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns a reference to the value associated with `key`, or `None`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Iterates over the `(key, value)` entries in insertion order.
    pub fn iter(&self) -> std::slice::Iter<'_, (String, Value)> {
        self.entries.iter()
    }
}

impl IntoIterator for ValueObject {
    type Item = (String, Value);
    type IntoIter = std::vec::IntoIter<(String, Value)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a> IntoIterator for &'a ValueObject {
    type Item = &'a (String, Value);
    type IntoIter = std::slice::Iter<'a, (String, Value)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl From<ValueObject> for Value {
    fn from(value: ValueObject) -> Self {
        Self::Object(value)
    }
}
