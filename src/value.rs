use std::sync::Arc;

use smol_str::SmolStr;

use crate::gen::{fn_enum_is_variant, fn_enum_variant_access, fn_enum_variant_try_into};


pub type Values<Ext> = Arc<[Value<Ext>]>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ExtValue<T>(pub T);

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Value<Ext> {
    Symbol(SmolStr),
    Int(i32),
    Float(f32),
    List(Values<Ext>),
    Ext(Ext),
}

impl<Ext> Value<Ext> {
    fn_enum_is_variant!(pub is_symbol, Symbol);
    fn_enum_is_variant!(pub is_int, Int);
    fn_enum_is_variant!(pub is_float, Float);
    fn_enum_is_variant!(pub is_list, List);
    fn_enum_is_variant!(pub is_ext, Ext);

    fn_enum_variant_access!(pub symbol -> &SmolStr, Self::Symbol(symbol) => symbol);
    fn_enum_variant_access!(pub int -> i32, Self::Int(value) => *value);
    fn_enum_variant_access!(pub float -> f32, Self::Float(value) => *value);
    fn_enum_variant_access!(pub list -> &Values<Ext>, Self::List(list) => list);
    fn_enum_variant_access!(pub ext -> &Ext, Self::Ext(ext) => ext);

    fn_enum_variant_try_into!(pub try_into_symbol -> SmolStr, Self::Symbol(symbol) => symbol);
    fn_enum_variant_try_into!(pub try_into_int -> i32, Self::Int(value) => value);
    fn_enum_variant_try_into!(pub try_into_float -> f32, Self::Float(value) => value);
    fn_enum_variant_try_into!(pub try_into_list -> Values<Ext>, Self::List(list) => list);
    fn_enum_variant_try_into!(pub try_into_ext -> Ext, Self::Ext(ext) => ext);
}

impl<Ext, T> FromIterator<T> for Value<Ext>
where
    T: Into<Self>,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::List(iter.into_iter().map(Into::into).collect())
    }
}

macro_rules! impl_value_from {
    ($source:ty, |$value:ident| $body:expr $(,)?) => {
        impl<Ext> From<$source> for Value<Ext> {
            fn from($value: $source) -> Self {
                $body
            }
        }
    };
    ($source:ty, $callback:path $(,)?) => {
        impl<Ext> From<$source> for Value<Ext> {
            fn from(value: $source) -> Self {
                $callback(value)
            }
        }
    }
}

impl_value_from!(SmolStr, Self::Symbol);
impl_value_from!(&SmolStr, |value| Self::Symbol(value.clone()));
impl_value_from!(&str, |value| Self::Symbol(value.into()));
impl_value_from!(i32, Self::Int);
impl_value_from!(f32, Self::Float);

impl<Ext> From<ExtValue<Ext>> for Value<Ext> {
    fn from(value: ExtValue<Ext>) -> Self {
        Self::Ext(value.0)
    }
}

impl<Ext, T> From<Vec<T>> for Value<Ext>
where
    T: Into<Self>,
{
    fn from(values: Vec<T>) -> Self {
        values.into_iter().collect()
    }
}

impl<Ext, T, const N: usize> From<[T; N]> for Value<Ext>
where
    T: Into<Self>,
{
    fn from(values: [T; N]) -> Self {
        values.into_iter().collect()
    }
}

macro_rules! impl_value_try_into {
    ($target:ty, $variant:pat => $body:expr) => {
        impl<Ext> TryInto<$target> for Value<Ext> {
            type Error = Self;

            fn try_into(self) -> Result<$target, Self> {
                if let $variant = self {
                    Ok($body)
                } else {
                    Err(self)
                }
            }
        }
    };
}

impl_value_try_into!(SmolStr, Self::Symbol(symbol) => symbol);
impl_value_try_into!(i32, Self::Int(value) => value);
impl_value_try_into!(f32, Self::Float(value) => value);

impl<Ext> TryInto<ExtValue<Ext>> for Value<Ext> {
    type Error = Self;

    fn try_into(self) -> Result<ExtValue<Ext>, Self> {
        if let Self::Ext(value) = self {
            Ok(ExtValue(value))
        } else {
            Err(self)
        }
    }
}

impl<Ext, T> TryInto<Vec<T>> for Value<Ext>
where
    T: TryFrom<Self>,
    Ext: Clone,
{
    type Error = Self;

    fn try_into(self) -> Result<Vec<T>, Self::Error> {
        if let Self::List(list) = &self {
            if let Ok(values) = list.iter().cloned().map(TryInto::try_into).collect() {
                return Ok(values);
            }
        }
        Err(self)
    }
}

pub trait IntoValues<Ext>: Sized {
    fn into_values<C>(self) -> C
    where
        C: FromIterator<Value<Ext>>;
}

impl<Ext, T, const N: usize> IntoValues<Ext> for [T; N]
where
    T: Into<Value<Ext>>,
{
    fn into_values<C>(self) -> C
    where
        C: FromIterator<Value<Ext>>,
    {
        self.into_iter().map(Into::into).collect()
    }
}

impl<Ext, T> IntoValues<Ext> for Vec<T>
where
    T: Into<Value<Ext>>,
{
    fn into_values<C>(self) -> C
    where
        C: FromIterator<Value<Ext>>,
    {
        self.into_iter().map(Into::into).collect()
    }
}

macro_rules! impl_tuple_into_values_next {
    () => {};
    ($first:ident $($rest:ident)*) => {
        impl_tuple_into_values!($($rest)*);
    }
}

macro_rules! impl_tuple_into_values {
    ($( $param:ident )*) => {
        impl<Ext, $($param),*> IntoValues<Ext> for ($($param,)*)
        where
            $(
                $param: Into<Value<Ext>>,
            )*
        {
            fn into_values<C>(self) -> C
            where
                C: FromIterator<Value<Ext>>,
            {
                #[allow(non_snake_case)]
                let ($($param,)*) = self;
                [$($param.into()),*].into_iter().collect()
            }
        }
        impl_tuple_into_values_next!($($param)*);
    }
}

impl_tuple_into_values!(T15 T14 T13 T12 T11 T10 T9 T8 T7 T6 T5 T4 T3 T2 T1 T0);

pub trait TryFromValues<Ext>: Sized {
    const ARITY: usize;

    fn try_from_values<I>(values: I) -> Option<Self>
    where
        I: IntoIterator<Item = Value<Ext>>;
}

impl<Ext, T, const N: usize> TryFromValues<Ext> for [T; N]
where
    Value<Ext>: TryInto<T>,
    //T: TryFrom<Value<Ext>>,
{
    const ARITY: usize = N;

    fn try_from_values<I>(values: I) -> Option<Self>
    where
        I: IntoIterator<Item = Value<Ext>>,
    {
        let mut values = values.into_iter().fuse();
        let values: [_; N] = std::array::from_fn(|_| {
            values.next().and_then(|value| value.try_into().ok())
        });
        if values.iter().any(|value| value.is_none()) {
            return None;
        }
        Some(values.map(|value| value.unwrap()))
    }
}

macro_rules! impl_tuple_try_from_values_next {
    () => {};
    ($first:ident $($rest:ident)*) => {
        impl_tuple_try_from_values!($($rest)*);
    }
}

macro_rules! const_arity {
    () => { 0 };
    ($first:ident $($rest:ident)*) => {
        1 + const_arity!($($rest)*)
    }
}

macro_rules! impl_tuple_try_from_values {
    ($( $param:ident )*) => {
        impl<Ext, $($param),*> TryFromValues<Ext> for ($($param,)*)
        where
            $(
                Value<Ext>: TryInto<$param>,
            )*
        {
            const ARITY: usize = const_arity!($($param)*);

            fn try_from_values<I>(values: I) -> Option<Self>
            where
                I: IntoIterator<Item = Value<Ext>>,
            {
                #[allow(unused)]
                let mut iter = values.into_iter();
                let tuple = ($(
                    { let _:$param; iter.next()?.try_into().ok()? },
                )*);
                if iter.next().is_none() {
                    Some(tuple)
                } else {
                    None
                }
            }
        }
        impl_tuple_try_from_values_next!($($param)*);
    };
}

impl_tuple_try_from_values!(T15 T14 T13 T12 T11 T10 T9 T8 T7 T6 T5 T4 T3 T2 T1 T0);
