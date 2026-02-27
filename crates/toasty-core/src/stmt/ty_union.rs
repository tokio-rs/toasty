use super::Type;

/// A set of types representing the possible result types of a match expression.
///
/// `TypeUnion` enforces the set invariant: inserting a type that is already
/// present is a no-op. Order is not significant; two `TypeUnion` values are
/// equal if they contain the same types regardless of insertion order.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeUnion {
    // Invariant: no duplicates.
    types: Vec<Type>,
}

impl TypeUnion {
    pub fn new() -> Self {
        Self { types: Vec::new() }
    }

    /// Insert `ty` if it is not already present. Returns whether it was inserted.
    ///
    /// `Type::Unknown` is skipped — it carries no type information and should
    /// not widen the union (e.g. an `Expr::Error` branch in a Match).
    pub fn insert(&mut self, ty: Type) -> bool {
        if matches!(ty, Type::Unknown) {
            return false;
        }
        if self.types.contains(&ty) {
            return false;
        }
        self.types.push(ty);
        true
    }

    pub fn contains(&self, ty: &Type) -> bool {
        self.types.contains(ty)
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Type> {
        self.types.iter()
    }

    /// Collapse the union into a single `Type`.
    ///
    /// - 0 members → `Type::Null` (no type information)
    /// - 1 member  → that type directly (no union needed)
    /// - 2+ members → `Type::Union(self)`
    pub fn simplify(self) -> Type {
        match self.types.len() {
            0 => Type::Null,
            1 => self.types.into_iter().next().unwrap(),
            _ => Type::Union(self),
        }
    }
}

impl Default for TypeUnion {
    fn default() -> Self {
        Self::new()
    }
}

/// Set equality: two unions are equal iff they contain the same types,
/// regardless of insertion order.
impl PartialEq for TypeUnion {
    fn eq(&self, other: &Self) -> bool {
        self.types.len() == other.types.len() && self.types.iter().all(|t| other.types.contains(t))
    }
}

impl Eq for TypeUnion {}

impl IntoIterator for TypeUnion {
    type Item = Type;
    type IntoIter = std::vec::IntoIter<Type>;

    fn into_iter(self) -> Self::IntoIter {
        self.types.into_iter()
    }
}

impl<'a> IntoIterator for &'a TypeUnion {
    type Item = &'a Type;
    type IntoIter = std::slice::Iter<'a, Type>;

    fn into_iter(self) -> Self::IntoIter {
        self.types.iter()
    }
}
