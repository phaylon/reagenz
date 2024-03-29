

macro_rules! fn_enum_is_variant {
    ($public:vis $name:ident, $variant:ident $(,)?) => {
        $public fn $name(&self) -> bool {
            matches!(self, Self::$variant { .. })
        }
    };
}

pub(crate) use fn_enum_is_variant;

macro_rules! fn_enum_variant_access {
    ($public:vis $name:ident -> $output:ty, $variant:pat => $body:expr $(,)?) => {
        $public fn $name(&self) -> Option<$output> {
            if let $variant = self {
                Some($body)
            } else {
                None
            }
        }
    };
}

pub(crate) use fn_enum_variant_access;

macro_rules! fn_enum_variant_try_into {
    ($public:vis $name:ident -> $output:ty, $variant:pat => $body:expr $(,)?) => {
        $public fn $name(self) -> Result<$output, Self> {
            if let $variant = self {
                Ok($body)
            } else {
                Err(self)
            }
        }
    };
}

pub(crate) use fn_enum_variant_try_into;

macro_rules! enum_class {
    ($public:vis $name:ident { $($variant:ident $( = $default:ty)?),* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $public enum $name<$($variant $( = $default)?),*> {
            $(
                $variant($variant),
            )*
        }

        #[allow(unused)]
        impl<$($variant),*> $name<$($variant),*> {
            pub fn to_value<T>(
                &self,
                value: T,
            ) -> $name<$($crate::gen::param_replace!(T, $variant)),*> {
                match self {
                    $(
                        Self::$variant(_) => $name::$variant(value),
                    )*
                }
            }

            pub fn as_ref(&self) -> $name<$(& $variant),*> {
                match self {
                    $(
                        Self::$variant(value) => $name::$variant(value),
                    )*
                }
            }
        }

        #[allow(unused)]
        impl<E, $($variant),*> $name<$(Result<$variant, E>),*> {
            pub fn lift(self) -> Result<$name<$($variant),*>, E> {
                match self {
                    $(
                        Self::$variant(Ok(value)) => Ok($name::$variant(value)),
                        Self::$variant(Err(error)) => Err(error),
                    )*
                }
            }
        }

        #[allow(unused)]
        impl<$($variant),*> $name<$(Option<$variant>),*> {
            pub fn lift(self) -> Option<$name<$($variant),*>> {
                match self {
                    $(
                        Self::$variant(Some(value)) => Some($name::$variant(value)),
                        Self::$variant(None) => None,
                    )*
                }
            }
        }

        impl<T> std::ops::Deref for $name<$( $crate::gen::param_replace!(T, $variant) ),*> {
            type Target = T;

            fn deref(&self) -> &Self::Target {
                match self {
                    $(
                        Self::$variant(v) => v,
                    )*
                }
            }
        }

        #[allow(unused)]
        impl<T> $name<$( $crate::gen::param_replace!(T, $variant) ),*> {
            pub fn into_inner(self) -> T {
                match self {
                    $(
                        Self::$variant(value) => value,
                    )*
                }
            }

            pub fn map<F, X>(
                self,
                mapv: F,
            ) -> $name<$( $crate::gen::param_replace!(X, $variant) ),*>
            where
                F: FnOnce(T) -> X,
            {
                match self {
                    $(
                        Self::$variant(v) => $name::$variant(mapv(v)),
                    )*
                }
            }
        }
    };
}

pub(crate) use enum_class;

macro_rules! param_replace {
    ($param:ty, $take:ident) => { $param }
}

pub(crate) use param_replace;

macro_rules! smol_str_wrapper {
    ($public:vis $name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $public struct $name(SmolStr);

        #[allow(unused)]
        impl $name {
            $public fn to_smol_str(&self) -> SmolStr {
                self.0.clone()
            }

            $public fn as_smol_str(&self) -> &SmolStr {
                &self.0
            }
        }

        impl std::ops::Deref for $name {
            type Target = SmolStr;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

pub(crate) use smol_str_wrapper;