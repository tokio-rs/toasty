use crate::{
    schema::{
        app::{self, Field, FieldId, Model},
        db::ColumnId,
    },
    stmt::{Expr, Value},
};

use indexmap::Equivalent;
use std::{
    fmt,
    hash::{Hash, Hasher},
    ops,
};

#[derive(Clone, PartialEq, Eq)]
pub struct Projection {
    steps: Steps,
}

pub trait Project {
    fn project(self, projection: &Projection) -> Option<Expr>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Steps {
    /// References the projection base
    Identity,

    /// One field step
    Single([usize; 1]),

    /// Multi field steps
    Multi(Vec<usize>),
}

// Custom Hash implementation to ensure compatibility with Equivalent trait:
// - Single-step projections hash like their contained usize: hash(Projection([1])) == hash(1)
// - Multi-step projections hash like their slice: hash(Projection([1,2])) == hash([1,2])
// This allows IndexMap lookups with both usize and [usize] to work correctly.
impl Hash for Projection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.steps.hash(state);
    }
}

impl Hash for Steps {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Steps::Identity => {
                // Hash a discriminant for identity
                0u8.hash(state);
            }
            Steps::Single([index]) => {
                // Hash as a single usize (no length prefix, no array wrapping)
                // This makes hash(Projection([i])) == hash(i)
                index.hash(state);
            }
            Steps::Multi(indices) => {
                // Hash as a slice: this includes length and elements
                // This makes hash(Projection([i,j])) == hash([i,j])
                indices.as_slice().hash(state);
            }
        }
    }
}

pub struct Iter<'a>(std::slice::Iter<'a, usize>);

impl Projection {
    pub const fn identity() -> Self {
        Self {
            steps: Steps::Identity,
        }
    }

    /// The path references the root (i.e. the projection base)
    pub const fn is_identity(&self) -> bool {
        matches!(self.steps, Steps::Identity)
    }

    pub fn single(step: usize) -> Self {
        Self {
            steps: Steps::Single([step]),
        }
    }

    /// Mostly here for `const`
    pub const fn from_index(index: usize) -> Self {
        Self {
            steps: Steps::Single([index]),
        }
    }

    pub fn as_slice(&self) -> &[usize] {
        self.steps.as_slice()
    }

    pub fn push(&mut self, step: usize) {
        match &mut self.steps {
            Steps::Identity => {
                self.steps = Steps::Single([step]);
            }
            Steps::Single([first]) => {
                self.steps = Steps::Multi(vec![*first, step]);
            }
            Steps::Multi(steps) => {
                steps.push(step);
            }
        }
    }

    pub fn resolve_field<'a>(&self, schema: &'a app::Schema, expr_self: &'a Model) -> &'a Field {
        self.steps.resolve_field(schema, expr_self)
    }

    pub fn resolves_to(&self, other: impl Into<Self>) -> bool {
        let other = other.into();
        *self == other
    }
}

impl ops::Deref for Projection {
    type Target = [usize];

    fn deref(&self) -> &Self::Target {
        self.steps.as_slice()
    }
}

impl ops::DerefMut for Projection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.steps {
            Steps::Identity => &mut [],
            Steps::Single(step) => &mut step[..],
            Steps::Multi(steps) => &mut steps[..],
        }
    }
}

impl<'a> IntoIterator for &'a Projection {
    type Item = usize;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self[..].iter())
    }
}

impl From<&Field> for Projection {
    fn from(value: &Field) -> Self {
        Self::single(value.id.index)
    }
}

impl From<FieldId> for Projection {
    fn from(value: FieldId) -> Self {
        Self::single(value.index)
    }
}

impl From<ColumnId> for Projection {
    fn from(value: ColumnId) -> Self {
        Self::single(value.index)
    }
}

impl From<usize> for Projection {
    fn from(value: usize) -> Self {
        Self::single(value)
    }
}

impl From<&[usize]> for Projection {
    fn from(value: &[usize]) -> Self {
        match value {
            [] => Self::identity(),
            [value] => Self::single(*value),
            value => Self {
                steps: Steps::Multi(value.into()),
            },
        }
    }
}

impl<const N: usize> From<[usize; N]> for Projection {
    fn from(value: [usize; N]) -> Self {
        Self::from(&value[..])
    }
}

impl fmt::Debug for Projection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_tuple("Projection");

        if self.is_identity() {
            f.field(&"identity");
        } else {
            for field in &self[..] {
                f.field(&field);
            }
        }

        f.finish()
    }
}

impl Steps {
    fn as_slice(&self) -> &[usize] {
        match self {
            Self::Identity => &[],
            Self::Single(step) => &step[..],
            Self::Multi(steps) => &steps[..],
        }
    }

    fn resolve_field<'a>(&self, schema: &'a app::Schema, expr_self: &'a Model) -> &'a Field {
        use crate::schema::app::FieldTy::*;

        let [first, rest @ ..] = self.as_slice() else {
            panic!("need at most one path step")
        };
        let mut projected = &expr_self.fields[*first];

        for step in rest {
            let target = match &projected.ty {
                Primitive(..) => panic!("failed to resolve path"),
                Embedded(_) => {
                    // TODO: Handle path projection through embedded fields
                    todo!("embedded field path projection")
                }
                BelongsTo(belongs_to) => belongs_to.target(schema),
                HasMany(has_many) => has_many.target(schema),
                HasOne(_) => todo!(),
            };

            projected = &target.fields[*step];
        }

        projected
    }
}

impl Iterator for Iter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        self.0.next().copied()
    }
}

impl Project for Expr {
    fn project(self, projection: &Projection) -> Option<Expr> {
        Some(self.entry(projection)?.to_expr())
    }
}

impl Project for &Expr {
    fn project(self, projection: &Projection) -> Option<Expr> {
        Some(self.entry(projection)?.to_expr())
    }
}

impl Project for &&Expr {
    fn project(self, projection: &Projection) -> Option<Expr> {
        Some(self.entry(projection)?.to_expr())
    }
}

impl Project for Value {
    fn project(self, projection: &Projection) -> Option<Expr> {
        Some(self.entry(projection).to_expr())
    }
}

impl Project for &Value {
    fn project(self, projection: &Projection) -> Option<Expr> {
        Some(self.entry(projection).to_expr())
    }
}

impl Project for &&Value {
    fn project(self, projection: &Projection) -> Option<Expr> {
        Some(self.entry(projection).to_expr())
    }
}

// Allow using usize directly where Projection is expected (for single-step projections)
impl Equivalent<Projection> for usize {
    fn equivalent(&self, key: &Projection) -> bool {
        matches!(key.as_slice(), [index] if *index == *self)
    }
}

// Allow using &Projection where Projection is expected
impl Equivalent<Projection> for &Projection {
    fn equivalent(&self, key: &Projection) -> bool {
        *self == key
    }
}

// Allow using [usize] slices where Projection is expected
impl Equivalent<Projection> for [usize] {
    fn equivalent(&self, key: &Projection) -> bool {
        self == key.as_slice()
    }
}

// PartialEq implementations for ergonomic comparisons
impl PartialEq<usize> for Projection {
    fn eq(&self, other: &usize) -> bool {
        matches!(self.as_slice(), [index] if *index == *other)
    }
}

impl PartialEq<Projection> for usize {
    fn eq(&self, other: &Projection) -> bool {
        other == self
    }
}

impl PartialEq<[usize]> for Projection {
    fn eq(&self, other: &[usize]) -> bool {
        self.as_slice() == other
    }
}

impl PartialEq<Projection> for [usize] {
    fn eq(&self, other: &Projection) -> bool {
        other == self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projection_eq_usize() {
        let proj_single = Projection::from(5);
        let proj_multi = Projection::from([1, 2]);

        // Single-step projection == usize (both directions)
        assert_eq!(proj_single, 5);
        assert_eq!(5, proj_single);
        assert_ne!(proj_single, 3);
        assert_ne!(3, proj_single);

        // Multi-step projection != usize
        assert_ne!(proj_multi, 1);
        assert_ne!(1, proj_multi);
    }

    #[test]
    fn test_projection_eq_slice() {
        let proj_single = Projection::from(5);
        let proj_multi = Projection::from([1, 2, 3]);

        // Projection == [usize] slice (both directions)
        assert_eq!(proj_single, [5][..]);
        assert_eq!([5][..], proj_single);
        assert_eq!(proj_multi, [1, 2, 3][..]);
        assert_eq!([1, 2, 3][..], proj_multi);

        // Mismatches
        assert_ne!(proj_single, [1, 2][..]);
        assert_ne!([1, 2][..], proj_single);
    }

    #[test]
    fn test_projection_hash_compatibility() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn hash<T: Hash + ?Sized>(value: &T) -> u64 {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        }

        // Single-step projection should hash like the contained usize
        let proj_single = Projection::from(42);
        assert_eq!(hash(&proj_single), hash(&42_usize));

        // Multi-step projection should hash like the slice
        let proj_multi = Projection::from([1, 2, 3]);
        let slice: &[usize] = &[1, 2, 3];
        assert_eq!(hash(&proj_multi), hash(slice));

        // Verify this works with IndexMap
        use indexmap::IndexMap;
        let mut map = IndexMap::new();
        map.insert(Projection::from(5), "value");

        // Can look up with usize
        assert_eq!(map.get(&5_usize), Some(&"value"));

        // Can look up with single-element slice
        let slice_single: &[usize] = &[5];
        assert_eq!(map.get(slice_single), Some(&"value"));

        // Multi-step example
        map.insert(Projection::from([1, 2]), "multi");

        // Can look up with multi-element slice
        let slice_multi: &[usize] = &[1, 2];
        assert_eq!(map.get(slice_multi), Some(&"multi"));
    }
}
