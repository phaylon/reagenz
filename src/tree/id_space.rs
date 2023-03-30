
use std::ops::ControlFlow;
use std::sync::Arc;

use smol_str::SmolStr;

use crate::value::Value;

use super::{Index, IdMap, KindError, ArityError};
use super::outcome::{Outcome};
use super::script::{ActionRoot, NodeRoot};

pub type QueryFn<Ctx, Ext, Eff> = dyn for<'ctx, 'args, 'iter_fn> Fn(
    &'ctx Ctx,
    &'args [Value<Ext>],
    &'iter_fn mut dyn for<'iter> FnMut(
        &'iter mut dyn Iterator<Item = Value<Ext>>,
    ) -> Outcome<Ext, Eff>,
) -> Outcome<Ext, Eff>;

pub type GlobalFn<Ctx, Ext> = dyn Fn(&Ctx) -> Value<Ext>;
pub type EffectFn<Ctx, Ext, Eff> = dyn Fn(&Ctx, &[Value<Ext>]) -> Option<Eff>;
pub type CondFn<Ctx, Ext> = dyn Fn(&Ctx, &[Value<Ext>]) -> bool;

macro_rules! generate {
    {
        $(
            $field:ident: $kind:ident / $index:ident ($node:ty, $data:ty) => $describe:literal
        ),*
        $(,)?
    } => {
        $(
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $index(Index);

            impl From<$index> for Index {
                fn from(index: $index) -> Self {
                    index.0
                }
            }

            impl From<Index> for $index {
                fn from(index: Index) -> Self {
                    Self(index)
                }
            }

            impl<Ctx, Ext, Eff> IdSpaceIndex<Ctx, Ext, Eff> for $index {
                type Node = $node;

                const KIND: Kind = Kind::$kind;

                fn id_map(
                    ids: &IdSpace<Ctx, Ext, Eff>,
                ) -> &IdMap<Self::Node, usize> {
                    &ids.$field
                }

                fn id_map_mut(
                    ids: &mut IdSpace<Ctx, Ext, Eff>,
                ) -> &mut IdMap<Self::Node, usize> {
                    &mut ids.$field
                }
            }
        )*

        #[flagnum::flag(#[derive(Default)] pub Kinds)]
        pub enum Kind {
            $(
                $kind,
            )*
        }

        impl std::fmt::Display for Kind {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        Self::$kind => $describe.fmt(f),
                    )*
                }
            }
        }

        #[derive(derivative::Derivative)]
        #[derivative(Clone(bound=""), Default(bound=""))]
        pub struct IdSpace<Ctx, Ext, Eff> {
            $(
                $field: IdMap<$node, $data>,
            )*
        }

        impl<Ctx, Ext, Eff> IdSpace<Ctx, Ext, Eff> {
            pub fn kind(&self, name: &str) -> Option<Kind> {
                $(
                    if self.$field.find(name).is_some() {
                        return Some(Kind::$kind);
                    }
                )*
                None
            }
        }
    };
}

generate! {
    globals: Global/GlobalIdx (Arc<GlobalFn<Ctx, Ext>>, usize) => "a global",
    effects: Effect/EffectIdx (Arc<EffectFn<Ctx, Ext, Eff>>, usize) => "an effect",
    conditions: Cond/CondIdx (Arc<CondFn<Ctx, Ext>>, usize) => "a condition",
    queries: Query/QueryIdx (Arc<QueryFn<Ctx, Ext, Eff>>, usize) => "a query",
    action_roots: Action/ActionIdx (Arc<ActionRoot<Ext>>, usize) => "an action",
    node_roots: Node/NodeIdx (Arc<NodeRoot<Ext>>, usize) => "a node",
}

impl<Ctx, Ext, Eff> IdSpace<Ctx, Ext, Eff> {
    pub fn contains<Idx>(&self, name: &str) -> bool
    where
        Idx: IdSpaceIndex<Ctx, Ext, Eff>,
    {
        Idx::id_map(self).find(name).is_some()
    }

    pub fn resolve<Idx>(&self, name: &str, given: usize) -> Result<Idx, IdError>
    where
        Idx: IdSpaceIndex<Ctx, Ext, Eff>,
    {
        if let Some(index) = Idx::id_map(self).find(name) {
            let expected = *Idx::id_map(self).data(index);
            if given == expected {
                Ok(index.into())
            } else {
                Err(IdError::Arity(ArityError { given, expected }))
            }
        } else if let Some(given) = self.kind(name) {
            Err(IdError::Kind(KindError { expected: Idx::KIND.into(), given }))
        } else {
            Err(IdError::Unknown)
        }
    }

    pub fn get<Idx>(&self, index: Idx) -> &Idx::Node
    where
        Idx: IdSpaceIndex<Ctx, Ext, Eff>,
    {
        Idx::id_map(self).node(index.into())
    }

    pub fn set<Idx>(&mut self, name: SmolStr, node: Idx::Node, arity: usize) -> Result<Idx, Kind>
    where
        Idx: IdSpaceIndex<Ctx, Ext, Eff>,
    {
        if let Some(kind) = self.kind(&name) {
            Err(kind)
        } else {
            Ok(Idx::id_map_mut(self).set(name, node, arity).into())
        }
    }

    pub fn set_node<Idx>(&mut self, index: Idx, node: Idx::Node)
    where
        Idx: IdSpaceIndex<Ctx, Ext, Eff>,
    {
        Idx::id_map_mut(self).set_node(index.into(), node);
    }
}

pub trait IdSpaceIndex<Ctx, Ext, Eff>: From<Index> + Into<Index> {
    type Node;

    const KIND: Kind;

    fn id_map(ids: &IdSpace<Ctx, Ext, Eff>) -> &IdMap<Self::Node, usize>;

    fn id_map_mut(ids: &mut IdSpace<Ctx, Ext, Eff>) -> &mut IdMap<Self::Node, usize>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdError {
    Unknown,
    Kind(KindError),
    Arity(ArityError),
}
