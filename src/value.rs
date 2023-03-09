use derivative::Derivative;
use smol_str::SmolStr;
use num_traits::NumCast;

use crate::World;


pub type ValueIter<W> = dyn Iterator<Item = Value<W>>;

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
    fn_access!(Ext, &W::Value, is_ext, ext, |ext| ext);
    fn_access!(Symbol, &SmolStr, is_symbol, symbol, |sym| sym);
    fn_access!(Int, i64, is_int, int, |i| *i);
    fn_access!(Float, f64, is_float, float, |f| *f);

    fn_convert_num!(to_i64, i64);
    fn_convert_num!(to_f64, f64);
}