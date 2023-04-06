
mod gen;
mod str;
mod value;
mod tree;

pub use self::{
    value::{ExtValue, Value, Values, IntoValues, TryFromValues},
    str::{is_symbol, is_variable},
    tree::{
        BehaviorTree,
        ArityError, KindError, IdError,
        Kind, Kinds, KindsDisplay,
        outcome::{
            Outcome,
            Action,
        },
        builder::{
            BehaviorTreeBuilder,
        },
        script::{
            ScriptSource,
            ScriptError,
            CompileError,
            ConflictError,
        },
    },
};

#[macro_export]
macro_rules! cond_fn {
    (
        $ctx:pat $( , $arg:ident : $arg_ty:ty )*
        => $body:expr $(,)?
    ) => {
        ($crate::__count_usize!($($arg)*), |$ctx, args: &[$crate::Value<_>]| {
            let args = args.iter().cloned();
            let args: ($($arg_ty,)*) = match $crate::TryFromValues::try_from_values(args) {
                Some(values) => values,
                None => {
                    return false;
                },
            };
            let ($($arg,)*): ($($arg_ty,)*) = args;
            $body
        })
    }
}

#[macro_export]
macro_rules! effect_fn {
    (
        $ctx:pat $( , $arg:ident : $arg_ty:ty )*
        => $body:expr $(,)?
    ) => {
        ($crate::__count_usize!($($arg)*), |$ctx, args: &[$crate::Value<_>]| {
            let args = args.iter().cloned();
            let args: ($($arg_ty,)*) = match $crate::TryFromValues::try_from_values(args) {
                Some(values) => values,
                None => {
                    return None;
                },
            };
            let ($($arg,)*): ($($arg_ty,)*) = args;
            From::from($body)
        })
    }
}

#[macro_export]
macro_rules! query_fn {
    (
        $ctx:pat $( , $arg:ident : $arg_ty:ty )*
        => $body:expr $(,)?
    ) => {
        ($crate::__count_usize!($($arg)*), |$ctx, args: &[$crate::Value<_>], iter_fn| {
            let args = args.iter().cloned();
            let args: ($($arg_ty,)*) = match $crate::TryFromValues::try_from_values(args) {
                Some(values) => values,
                None => {
                    return iter_fn(&mut std::iter::empty());
                },
            };
            let ($($arg,)*) = args;
            let mut iter = IntoIterator::into_iter($body);
            iter_fn(&mut iter)
        })
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __count_usize {
    () => { 0usize };
    ($first:tt $($rest:tt)*) => {
        1usize + $crate::__count_usize!($($rest)*)
    };
}