use crate::{
    schema::{
        app::{Field, FieldId},
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
    Single([Step; 1]),

    /// Multi field steps
    Multi(Vec<Step>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    Field(usize),
    Index(usize),
    Variant(usize),
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

pub struct Iter<'a>(std::slice::Iter<'a, Step>);

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

    pub const fn field(index: usize) -> Projection {
        Projection {
            steps: Steps::Single([Step::Field(index)]),
        }
    }

    pub fn as_slice(&self) -> &[Step] {
        self.steps.as_slice()
    }

    pub fn push(&mut self, step: Step) {
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

    pub fn resolves_to(&self, other: impl Into<Self>) -> bool {
        let other = other.into();
        *self == other
    }
}

impl ops::Deref for Projection {
    type Target = [Step];

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
    type Item = Step;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self[..].iter())
    }
}

impl From<Step> for Projection {
    fn from(value: Step) -> Self {
        Projection {
            steps: Steps::Single([value]),
        }
    }
}

impl From<&Step> for Projection {
    fn from(value: &Step) -> Self {
        Projection {
            steps: Steps::Single([*value]),
        }
    }
}

impl From<&[Step]> for Projection {
    fn from(value: &[Step]) -> Self {
        match value {
            [] => Projection::identity(),
            [value] => Projection::from(value),
            value => Self {
                steps: Steps::Multi(value.into()),
            },
        }
    }
}

impl From<&Field> for Projection {
    fn from(value: &Field) -> Self {
        Step::Field(value.id.index).into()
    }
}

impl From<FieldId> for Projection {
    fn from(value: FieldId) -> Self {
        Step::Field(value.index).into()
    }
}

impl From<ColumnId> for Projection {
    fn from(value: ColumnId) -> Self {
        Step::Field(value.index).into()
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
    fn as_slice(&self) -> &[Step] {
        match self {
            Self::Identity => &[],
            Self::Single(step) => &step[..],
            Self::Multi(steps) => &steps[..],
        }
    }
}

impl Iterator for Iter<'_> {
    type Item = Step;

    fn next(&mut self) -> Option<Step> {
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

// Allow using &Projection where Projection is expected
impl Equivalent<Projection> for &Projection {
    fn equivalent(&self, key: &Projection) -> bool {
        *self == key
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
