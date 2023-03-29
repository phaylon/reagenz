use std::sync::Arc;

use crate::tree::id_space::{EffectIdx, GlobalIdx, QueryIdx, CondIdx, ActionIdx, NodeIdx};
use crate::value::Value;


pub type Nodes<Ext> = Arc<[Node<Ext>]>;
pub type ProtoValues<Ext> = Arc<[ProtoValue<Ext>]>;

pub type Patterns<Ext> = Arc<[Pattern<Ext>]>;

#[derive(Debug, Clone)]
pub struct ActionRoot<Ext> {
    pub effects: Arc<[(EffectIdx, ProtoValues<Ext>)]>,
    pub conditions: Nodes<Ext>,
    pub discovery: NodeRoot<Ext>,
    pub lexicals: usize,
}

impl<Ext> Default for ActionRoot<Ext> {
    fn default() -> Self {
        Self {
            effects: Arc::new([]),
            conditions: Arc::new([]),
            discovery: NodeRoot::default(),
            lexicals: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeRoot<Ext> {
    pub node: Node<Ext>,
    pub lexicals: usize,
}

impl<Ext> Default for NodeRoot<Ext> {
    fn default() -> Self {
        Self {
            node: Node::Failure,
            lexicals: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ProtoValue<Ext> {
    Global(GlobalIdx),
    Lexical(usize),
    Value(Value<Ext>),
    List(ProtoValues<Ext>),
}

#[derive(Debug, Clone)]
pub enum Node<Ext> {
    Failure,
    Dispatch(Dispatch, Nodes<Ext>),
    Ref(Ref, RefMode, ProtoValues<Ext>),
    Query(Pattern<Ext>, QueryIdx, ProtoValues<Ext>, QueryMode, Nodes<Ext>),
    Match(ProtoValues<Ext>, Patterns<Ext>, Nodes<Ext>),
}

impl<Ext> Node<Ext> {
    pub fn sequence(nodes: Nodes<Ext>) -> Self {
        Self::Dispatch(Dispatch::Sequence, nodes)
    }

    pub fn selection(nodes: Nodes<Ext>) -> Self {
        Self::Dispatch(Dispatch::Selection, nodes)
    }

    pub fn none(nodes: Nodes<Ext>) -> Self {
        Self::Dispatch(Dispatch::None, nodes)
    }

    pub fn visit(nodes: Nodes<Ext>) -> Self {
        Self::Dispatch(Dispatch::Visit, nodes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ref {
    Action(ActionIdx),
    Cond(CondIdx),
    Node(NodeIdx),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefMode {
    Query,
    Inherit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    Sequence,
    Selection,
    None,
    Visit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    Sequence,
    Selection,
    First,
    Last,
    Visit,
}

#[derive(Debug, Clone)]
pub enum Pattern<Ext> {
    Exact(Value<Ext>),
    Bind,
    Lexical(usize),
    Global(GlobalIdx),
    List(Patterns<Ext>),
}