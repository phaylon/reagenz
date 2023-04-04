
use std::sync::Arc;

use smol_str::SmolStr;

use crate::value::Value;

use super::{Index, IdMap, KindError, ArityError};
use super::outcome::{Outcome};
use super::script::{ActionRoot, NodeRoot};

pub type QueryFn<Ctx, Ext, Eff> = fn(
    &Ctx,
    &[Value<Ext>],
    &mut dyn FnMut(&mut dyn Iterator<Item = Value<Ext>>) -> Outcome<Ext, Eff>,
) -> Outcome<Ext, Eff>;
pub type GlobalFn<Ctx, Ext> = fn(&Ctx) -> Value<Ext>;
pub type EffectFn<Ctx, Ext, Eff> = fn(&Ctx, &[Value<Ext>]) -> Option<Eff>;
pub type CondFn<Ctx, Ext> = fn(&Ctx, &[Value<Ext>]) -> bool;

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
    globals: Global/GlobalIdx (GlobalFn<Ctx, Ext>, usize) => "a global",
    effects: Effect/EffectIdx (EffectFn<Ctx, Ext, Eff>, usize) => "an effect",
    conditions: Cond/CondIdx (CondFn<Ctx, Ext>, usize) => "a condition",
    queries: Query/QueryIdx (QueryFn<Ctx, Ext, Eff>, usize) => "a query",
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

    pub fn resolve_ref(&self, name: &str, given: usize) -> Result<RefIdx, IdError> {
        match self.kind(name) {
            Some(kind) => match kind {
                Kind::Action => self.resolve(name, given).map(RefIdx::Action),
                Kind::Node => self.resolve(name, given).map(RefIdx::Node),
                Kind::Cond => self.resolve(name, given).map(RefIdx::Cond),
                other => Err(IdError::Kind(KindError {
                    expected: [Kind::Action, Kind::Node, Kind::Cond].into(),
                    given: other,
                })),
            },
            None => Err(IdError::Unknown),
        }
    }

    pub fn actions(&self) -> impl Iterator<Item = ActionIdx> {
        self.action_roots.indices().map(Into::into)
    }

    pub fn action(&self, name: &str) -> Result<ActionIdx, IdError> {
        if let Some(index) = ActionIdx::id_map(self).find(name) {
            Ok(index.into())
        } else if let Some(given) = self.kind(name) {
            Err(IdError::Kind(KindError { expected: Kind::Action.into(), given }))
        } else {
            Err(IdError::Unknown)
        }
    }

    #[track_caller]
    pub fn action_name(&self, action: ActionIdx) -> &SmolStr {
        ActionIdx::id_map(self).name(action.into()).expect("action must be valid in this tree")
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

impl Kinds {
    pub fn display_connected(&self, connect: &'static str) -> KindsDisplay {
        KindsDisplay { kinds: *self, connect }
    }
}

#[derive(Clone, Copy)]
pub struct KindsDisplay {
    kinds: Kinds,
    connect: &'static str,
}

impl std::fmt::Display for KindsDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.kinds.is_empty() {
            write!(f, "none")
        } else if self.kinds.len() == 1 {
            self.kinds.into_iter().next().unwrap().fmt(f)
        } else {
            let mut items = self.kinds.into_iter();
            for _ in 0..(self.kinds.len() - 2) {
                write!(f, "{}, ", items.next().unwrap())?;
            }
            let last_a = items.next().unwrap();
            let last_b = items.next().unwrap();
            write!(f, "{} {} {}", last_a, self.connect, last_b)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefIdx {
    Action(ActionIdx),
    Node(NodeIdx),
    Cond(CondIdx),
}

pub trait IdSpaceIndex<Ctx, Ext, Eff>: From<Index> + Into<Index> {
    type Node;

    const KIND: Kind;

    fn id_map(ids: &IdSpace<Ctx, Ext, Eff>) -> &IdMap<Self::Node, usize>;

    fn id_map_mut(ids: &mut IdSpace<Ctx, Ext, Eff>) -> &mut IdMap<Self::Node, usize>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum IdError {
    #[error("Unknown identifier")]
    Unknown,
    #[error("Invalid kind: {_0}")]
    Kind(KindError),
    #[error("Wrong arity: {_0}")]
    Arity(ArityError),
}
