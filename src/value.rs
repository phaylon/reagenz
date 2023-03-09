use derivative::Derivative;
use smol_str::SmolStr;

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
    Symbol(Symbol),
    Int(i64),
    Float(f64),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SymbolError {
    Empty,
    Whitespace,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(SmolStr);

impl Symbol {
    pub fn verify(value: &str) -> Result<(), SymbolError> {
        if value.is_empty() {
            Err(SymbolError::Empty)
        } else if value.chars().any(char::is_whitespace) {
            Err(SymbolError::Whitespace)
        } else {
            Ok(())
        }
    }

    pub fn new<T>(value: T) -> Result<Self, SymbolError>
    where
        T: Into<SmolStr> + AsRef<str>,
    {
        Self::verify(value.as_ref())?;
        Ok(Self(value.into()))
    }
}

impl std::fmt::Debug for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.as_str().fmt(f)
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.as_str().fmt(f)
    }
}

impl std::borrow::Borrow<str> for Symbol {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl std::ops::Deref for Symbol {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl TryFrom<&str> for Symbol {
    type Error = SymbolError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&SmolStr> for Symbol {
    type Error = SymbolError;

    fn try_from(value: &SmolStr) -> Result<Self, Self::Error> {
        Self::verify(value)?;
        Ok(Self(value.clone()))
    }
}

impl From<&Symbol> for Symbol {
    fn from(value: &Symbol) -> Self {
        value.clone()
    }
}

impl TryFrom<std::borrow::Cow<'_, str>> for Symbol {
    type Error = SymbolError;

    fn try_from(value: std::borrow::Cow<'_, str>) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}