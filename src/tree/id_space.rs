
use std::sync::Arc;

use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::value::Value;

use super::{Index, IdMap, KindError, ArityError};
use super::script::{ActionRoot, NodeRoot};


pub type QueryVec<Ext> = SmallVec<[Value<Ext>; 16]>;

pub type GlobalFn<Ctx, Ext> = dyn Fn(&Ctx) -> Value<Ext>;
pub type EffectFn<Ctx, Ext, Eff> = dyn Fn(&Ctx, &[Value<Ext>]) -> Option<Eff>;
pub type QueryFn<Ctx, Ext> = dyn Fn(&Ctx, &[Value<Ext>]) -> QueryVec<Ext>;
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

            /*
            impl<Ctx, Ext, Eff> IdSpaceAccess<Ctx, Ext, Eff, $index> for IdSpace<Ctx, Ext, Eff> {
                type Node = $node;

                const KIND: Kind = Kind::$kind;

                fn id_space(&self) -> &Self {
                    self
                }

                fn id_map(&self) -> &IdMap<$node, $data> {
                    &self.$field
                }

                fn id_map_mut(&mut self) -> &mut IdMap<$node, $data> {
                    &mut self.$field
                }
            }
            */
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
    queries: Query/QueryIdx (Arc<QueryFn<Ctx, Ext>>, usize) => "a query",
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

#[derive(Debug, Clone)]
pub struct Signature {
    arity: usize,
}

pub trait IdSpaceIndex<Ctx, Ext, Eff>: From<Index> + Into<Index> {
    type Node;

    const KIND: Kind;

    fn id_map(ids: &IdSpace<Ctx, Ext, Eff>) -> &IdMap<Self::Node, usize>;

    fn id_map_mut(ids: &mut IdSpace<Ctx, Ext, Eff>) -> &mut IdMap<Self::Node, usize>;
}

/*
pub trait IdSpaceAccess<Ctx, Ext, Eff, Idx>
where
    Idx: From<Index> + Into<Index>,
{
    type Node;

    const KIND: Kind;

    fn id_space(&self) -> &IdSpace<Ctx, Ext, Eff>;

    fn id_map(&self) -> &IdMap<Self::Node, usize>;

    fn id_map_mut(&mut self) -> &mut IdMap<Self::Node, usize>;

    fn resolve(&self, name: &str, given: usize) -> Result<Idx, IdError> {
        if let Some(index) = self.id_map().find(name) {
            let expected = *self.id_map().data(index);
            if given == expected {
                Ok(index.into())
            } else {
                Err(IdError::Arity { given, expected })
            }
        } else if let Some(found) = self.id_space().kind(name) {
            Err(IdError::Kind { required: Self::KIND, found })
        } else {
            Err(IdError::Unknown)
        }
    }

    fn get(&self, index: Idx) -> &Self::Node {
        self.id_map().node(index.into())
    }

    fn set(&mut self, name: SmolStr, node: Self::Node, arity: usize) -> Result<Idx, Kind> {
        if let Some(kind) = self.id_space().kind(&name) {
            Err(kind)
        } else {
            Ok(self.id_map_mut().set(name, node, arity).into())
        }
    }

    fn set_node(&mut self, index: Idx, node: Self::Node) {
        self.id_map_mut().set_node(index.into(), node);
    }
}
*/

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdError {
    Unknown,
    Kind(KindError),
    Arity(ArityError),
}
