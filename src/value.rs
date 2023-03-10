use std::sync::Arc;

use derivative::Derivative;
use ramble::{Item, Num, ItemKind, GroupKind};
use smallvec::SmallVec;
use smol_str::SmolStr;
use num_traits::NumCast;

use crate::World;
use crate::loader::is_reserved_char;


pub type ValueIter<W> = dyn Iterator<Item = Value<W>>;

pub type Args<T> = SmallVec<[T; 16]>;

pub type List<T> = Arc<[T]>;

#[derive(Derivative)]
#[derivative(
    Clone(bound=""),
    Debug(bound="W::Value: std::fmt::Debug"),
    PartialEq(bound="W::Value: PartialEq"),
)]
pub enum Value<W: World> {
    Ext(W::Value),
    Symbol(SmolStr),
    Int(i64),
    Float(f64),
    List(List<Self>),
}

macro_rules! fn_access {
    ($variant:ident, $access:ty, $pred:ident, $getter:ident, |$value:ident| $get:expr $(,)?) => {
        pub fn $getter(&self) -> Option<$access> {
            if let Self::$variant($value) = self {
                Some($get)
            } else {
                None
            }
        }

        pub fn $pred(&self) -> bool {
            matches!(self, Self::$variant(_))
        }
    }
}

macro_rules! fn_convert_num {
    ($name:ident, $output:ty $(,)?) => {
        pub fn $name(&self) -> Option<$output> {
            match self {
                Self::Int(i) => NumCast::from(*i),
                Self::Float(f) => NumCast::from(*f),
                _ => None,
            }
        }
    }
}

impl<W> Value<W>
where
    W: World,
{
    pub(crate) fn try_from_item(item: &Item) -> Option<Self> {
        match item.kind {
            ItemKind::Word(ref word) => Some(Self::Symbol(word.clone())),
            ItemKind::Num(Num::Int(i)) => Some(Self::Int(i)),
            ItemKind::Num(Num::Float(f)) => Some(Self::Float(f)),
            ItemKind::Group(GroupKind::Brackets, ref items) => items.iter()
                .map(Self::try_from_item)
                .collect::<Option<Arc<[_]>>>()
                .map(Self::List),
            ItemKind::Punctuation(_) | ItemKind::Group(_, _) => None,
        }
    }

    fn_access!(Ext, &W::Value, is_ext, ext, |ext| ext);
    fn_access!(Symbol, &SmolStr, is_symbol, symbol, |sym| sym);
    fn_access!(Int, i64, is_int, int, |i| *i);
    fn_access!(Float, f64, is_float, float, |f| *f);
    fn_access!(List, &[Self], is_list, list, |values| values);

    fn_convert_num!(to_i64, i64);
    fn_convert_num!(to_f64, f64);
}

impl<W, V> FromIterator<V> for Value<W>
where
    W: World,
    V: Into<Self>,
{
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        Self::List(iter.into_iter().map(Into::into).collect())
    }
}

macro_rules! impl_value_from {
    ($source:ty, |$value:ident| $create:expr) => {
        impl<W> From<$source> for Value<W> where W: World {
            fn from($value: $source) -> Self {
                $create
            }
        }
    }
}

impl_value_from!(i64, |v| Self::Int(v));
impl_value_from!(i32, |v| Self::Int(v.into()));
impl_value_from!(f64, |v| Self::Float(v));
impl_value_from!(f32, |v| Self::Float(v.into()));
impl_value_from!(SmolStr, |v| Self::Symbol(v));
impl_value_from!(&SmolStr, |v| Self::Symbol(v.clone()));

impl<W, T> From<Vec<T>> for Value<W>
where
    W: World,
    T: Into<Self>,
{
    fn from(values: Vec<T>) -> Self {
        values.into_iter().collect()
    }
}

impl<W, T, const N: usize> From<[T; N]> for Value<W>
where
    W: World,
    T: Into<Self>,
{
    fn from(values: [T; N]) -> Self {
        values.into_iter().collect()
    }
}

pub trait StrExt {
    fn as_str(&self) -> &str;

    fn is_variable(&self) -> bool {
        let string = self.as_str();
        string.len() > 1
        && string.starts_with('$')
        && string[1..].is_symbol()
    }

    fn is_symbol(&self) -> bool {
        let string = self.as_str();
        !string.is_empty()
        && string.chars().all(|c| !c.is_whitespace() && !is_reserved_char(c))
    }
}

impl StrExt for str {
    fn as_str(&self) -> &str {
        self
    }
}

impl StrExt for SmolStr {
    fn as_str(&self) -> &str {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_variable() {
        for ok in [
            "$a",
            "$abc",
            "$ab-cd",
            "$*abcd*",
            "$+",
            "$@abc",
            "$a.b.c",
            "$a/b/c",
        ] {
            assert!(ok.is_variable(), "string {ok:?} should be a valid variable");
        }

        for fail in [
            "",
            "a",
            "abc$",
            "$abc$",
            "$a b",
            "$a;b",
            "$a(b",
            "$a:b",
            "$a!b",
            "$a?b",
        ] {
            assert!(!fail.is_variable(), "string {fail:?} should not be a valid variable");
        }
    }

    #[test]
    fn is_symbol() {
        for ok in [
            "a",
            "abc",
            "ab-cd",
            "*abcd*",
            "+",
            "@abc",
            "a.b.c",
            "a/b/c",
        ] {
            assert!(ok.is_symbol(), "string {ok:?} should be a valid symbol");
        }

        for fail in [
            "",
            "$abc",
            "abc$",
            "a b",
            "a;b",
            "a(b",
            "a:b",
            "a!b",
            "a?b",
        ] {
            assert!(!fail.is_symbol(), "string {fail:?} should not be a valid symbol");
        }
    }
}