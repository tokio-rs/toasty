// TODO: remove
#![allow(dead_code)]

use super::*;

use std::ops;

macro_rules! grouped {
    ($(#[$meta:meta])* $name:ident { $field:ident, $open:ty, $close:ty }) => {

        $(#[$meta])*
        #[derive(Debug, Clone)]
        #[non_exhaustive]
        pub(crate) struct $name<T, S> {
            /// The opening punctuation.
            pub open: $open,

            /// Values in the group
            pub $field: Punctuated<T, S>,

            /// The closing punctuation.
            pub close: $close,
        }

        impl<T, S> $name<T, S> {
            /// Test if group is empty.
            pub fn is_empty(&self) -> bool {
                self.$field.is_empty()
            }

            /// Get the length of elements in the group.
            pub fn len(&self) -> usize {
                self.$field.len()
            }

            /// Get the first element in the group.
            pub fn first(&self) -> Option<&T> {
                self.$field.first()
            }

            /// Get the last element in the group.
            pub fn last(&self) -> Option<&T> {
                self.$field.last()
            }

            /// Iterate over elements in the group.
            pub fn iter(&self) -> punctuated::Iter<'_, T, S> {
                self.$field.iter()
            }
        }

        impl<T, S> ops::Index<usize> for $name<T, S> {
            type Output = T;

            fn index(&self, index: usize) -> &T {
                &self.$field[index]
            }
        }

        impl<'a, T, S> IntoIterator for &'a $name<T, S> {
            type Item = &'a T;
            type IntoIter = punctuated::Iter<'a, T, S>;

            fn into_iter(self) -> Self::IntoIter {
                self.iter()
            }
        }

        impl<T, S> Parse for $name<T, S>
        where
            T: Parse,
            S: Peek,
        {
            fn parse(p: &mut Parser<'_>) -> Result<Self> {
                let open = p.parse()?;

                let mut items = Vec::new();

                // TODO: use Punctated impl
                while !p.is_next::<$close>() {
                    let expr = p.parse()?;
                    let sep = p.parse::<Option<S>>()?;
                    let is_end = sep.is_none();
                    items.push((expr, sep));

                    if is_end {
                        break;
                    }
                }

                let close = p.parse()?;

                Ok(Self {
                    open,
                    $field: Punctuated { items },
                    close,
                })
            }
        }
    }
}

grouped! {
    /// Parse something parenthesis, that is separated by `((T, S?)*)`.
    Parenthesized { parenthesized, punct::LParen, punct::RParen }
}

grouped! {
    /// Parse something bracketed, that is separated by `[(T, S?)*]`.
    Bracketed { bracketed, punct::LBracket, punct::RBracket }
}

// grouped! {
//     /// Parse something braced, that is separated by `{(T, S?)*}`.
//     Braced { braced, punct::LCurly, punct::RCurly }
// }

grouped! {
    /// Parse something bracketed, that is separated by `<(T, S?)*>`.
    AngleBracketed { angle_bracketed, punct::Lt, punct::Gt }
}
