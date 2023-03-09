use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use derivative::Derivative;
use smol_str::SmolStr;

use crate::World;
use crate::loader::{LoadError, load_str};
use crate::value::{Value, ValueIter};


type SymbolMap = HashMap<SmolStr, (usize, SymbolInfo)>;

pub(crate) type NodeHook<W> = dyn Fn(&Context<'_, W>, &[Value<W>]) -> Outcome<W>;
pub(crate) type EffectHook<W> = dyn Fn(&Context<'_, W>, &[Value<W>]) -> Option<<W as World>::Effect>;
pub(crate) type QueryHook<W> = dyn Fn(&Context<'_, W>, &[Value<W>]) -> Box<ValueIter<W>>;
pub(crate) type GetterHook<W> = dyn Fn(&Context<'_, W>, &[Value<W>]) -> Option<Value<W>>;

#[derive(Derivative)]
#[derivative(
    Debug(bound="Action<W>: std::fmt::Debug"),
    Clone(bound="Action<W>: Clone"),
    PartialEq(bound="Action<W>: PartialEq"), Eq(bound="Action<W>: Eq"),
)]
pub enum Outcome<W: World> {
    Success,
    Failure,
    Action(Action<W>),
}

impl<W> Outcome<W>
where
    W: World,
{
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failure)
    }

    pub fn action(&self) -> Option<&Action<W>> {
        if let Self::Action(action) = self {
            Some(action)
        } else {
            None
        }
    }
}

#[derive(Derivative)]
#[derivative(
    Default(bound=""),
    Debug(bound="W::Effect: std::fmt::Debug"),
    Clone(bound="W::Effect: Clone"),
    PartialEq(bound="W::Effect: PartialEq"), Eq(bound="W::Effect: Eq"),
)]
pub struct Action<W: World> {
    pub effects: Vec<W::Effect>,
}

pub struct System<W: World> {
    symbols: SymbolMap,
    nodes: Vec<Box<NodeHook<W>>>,
    effects: Vec<Box<EffectHook<W>>>,
    queries: Vec<Box<QueryHook<W>>>,
    getters: Vec<Box<GetterHook<W>>>,
}

impl<W> System<W>
where
    W: World,
{
    pub fn new() -> Self {
        Self {
            symbols: SymbolMap::new(),
            nodes: Vec::new(),
            effects: Vec::new(),
            queries: Vec::new(),
            getters: Vec::new(),
        }
    }

    pub fn load_from_str(self, content: &str) -> Result<Self, LoadError> {
        load_str(content, self, SymbolSourceProto::Api)
    }

    pub fn symbols(&self) -> impl Iterator<Item = &SmolStr> + '_ {
        self.symbols.keys()
    }

    pub fn symbol<T>(&self, value: T) -> Option<&SymbolInfo>
    where
        T: AsRef<str>,
    {
        self.symbols.get(value.as_ref()).map(|(_, info)| info)
    }

    pub(crate) fn symbol_index(&self, value: &str) -> Option<usize> {
        self.symbols.get(value).map(|(index, _)| *index)
    }

    pub fn register_effect<S, F, const N: usize>(
        &mut self,
        name: S,
        body: F,
    ) -> Result<(), SystemSymbolError>
    where
        S: Into<SmolStr>,
        F: Fn(&Context<'_, W>, &[Value<W>; N]) -> Option<W::Effect> + 'static,
    {
        register_api_symbol(
            &mut self.symbols,
            &mut self.effects,
            SymbolSource::Api,
            name.into(),
            SymbolKind::Effect,
            body,
        )
    }

    pub fn register_query<S, F, const N: usize>(
        &mut self,
        name: S,
        body: F,
    ) -> Result<(), SystemSymbolError>
    where
        S: Into<SmolStr>,
        F: Fn(&Context<'_, W>, &[Value<W>; N]) -> Box<ValueIter<W>> + 'static,
    {
        register_api_symbol(
            &mut self.symbols,
            &mut self.queries,
            SymbolSource::Api,
            name.into(),
            SymbolKind::Query,
            body,
        )
    }

    pub fn register_getter<S, F, const N: usize>(
        &mut self,
        name: S,
        body: F,
    ) -> Result<(), SystemSymbolError>
    where
        S: Into<SmolStr>,
        F: Fn(&Context<'_, W>, &[Value<W>; N]) -> Option<Value<W>> + 'static,
    {
        register_api_symbol(
            &mut self.symbols,
            &mut self.getters,
            SymbolSource::Api,
            name.into(),
            SymbolKind::Getter,
            body,
        )
    }

    pub fn register_node<S, F, const N: usize>(
        &mut self,
        name: S,
        body: F,
    ) -> Result<(), SystemSymbolError>
    where
        S: Into<SmolStr>,
        F: Fn(&Context<'_, W>, &[Value<W>; N]) -> Outcome<W> + 'static,
    {
        register_api_symbol(
            &mut self.symbols,
            &mut self.nodes,
            SymbolSource::Api,
            name.into(),
            SymbolKind::Node,
            body,
        )
    }

    pub(crate) fn register_node_raw(
        &mut self,
        name: SmolStr,
        info: SymbolInfo,
        hook: Box<NodeHook<W>>,
    ) -> Result<(), SystemSymbolError> {
        register_symbol_hook(&mut self.symbols, &mut self.nodes, name, info, hook)
    }

    pub(crate) fn replace_node_hook_raw(&mut self, name: &str, hook: Box<NodeHook<W>>) {
        let index = self.symbol_index(name).expect("can only replace known symbols");
        self.nodes[index] = hook;
    }
}

impl<W> Default for System<W>
where
    W: World,
{
    fn default() -> Self {
        Self::new()
    }
}

fn register_symbol_hook<W, R>(
    symbols: &mut SymbolMap,
    hooks: &mut Vec<Box<dyn Fn(&Context<'_, W>, &[Value<W>]) -> R>>,
    name: SmolStr,
    info: SymbolInfo,
    hook: Box<dyn Fn(&Context<'_, W>, &[Value<W>]) -> R>,
) -> Result<(), SystemSymbolError>
where
    W: World,
{
    if let Some((_, previous)) = symbols.get(&name).cloned() {
        Err(SystemSymbolError::Conflict { previous, current: info })
    } else {
        let index = hooks.len();
        hooks.push(hook);
        symbols.insert(name, (index, info));
        Ok(())
    }
}

fn register_api_symbol<W, R, F, const N: usize>(
    symbols: &mut SymbolMap,
    hooks: &mut Vec<Box<dyn Fn(&Context<'_, W>, &[Value<W>]) -> R>>,
    source: SymbolSource,
    name: SmolStr,
    kind: SymbolKind,
    body: F,
) -> Result<(), SystemSymbolError>
where
    W: World,
    F: Fn(&Context<'_, W>, &[Value<W>; N]) -> R + 'static,
{
    let info = SymbolInfo { source, kind, arity: N };
    register_symbol_hook(symbols, hooks, name.clone(), info, {
        Box::new(move |ctx, arguments: &[Value<W>]| {
            let arguments: &[Value<W>; N] = match arguments.try_into() {
                Ok(values) => values,
                Err(_) => panic!(
                    "reached {:?} hook for {:?} with wrong arity {}",
                    kind,
                    &name,
                    format_args!("{} instead of {}", arguments.len(), N),
                ),
            };
            body(ctx, arguments)
        })
    })
}

#[derive(Derivative)]
#[derivative(
    Clone(bound=""),
)]
pub struct Context<'a, W: World> {
    world: &'a W,
    system: &'a System<W>,
    mode: ContextMode,
}

impl<'a, W> Context<'a, W>
where
    W: World,
{
    pub fn new(world: &'a W, system: &'a System<W>, mode: ContextMode) -> Self {
        Self { world, system, mode }
    }

    pub fn system(&self) -> &'a System<W> {
        self.system
    }

    pub fn world(&self) -> &'a W {
        self.world
    }

    pub fn is_active(&self) -> bool {
        self.mode == ContextMode::Active
    }

    pub fn to_inactive(&self) -> Self {
        Self {
            world: self.world,
            system: self.system,
            mode: ContextMode::Inactive,
        }
    }

    pub(crate) fn run_raw(&self, index: usize, arguments: &[Value<W>]) -> Outcome<W> {
        self.system.nodes[index](self, arguments)
    }

    pub(crate) fn query_raw(&self, index: usize, arguments: &[Value<W>]) -> Box<ValueIter<W>> {
        self.system.queries[index](self, arguments)
    }

    pub(crate) fn get_raw(&self, index: usize, arguments: &[Value<W>]) -> Option<Value<W>> {
        self.system.getters[index](self, arguments)
    }

    pub(crate) fn effect_raw(&self, index: usize, arguments: &[Value<W>]) -> Option<W::Effect> {
        self.system.effects[index](self, arguments)
    }

    pub fn run<T>(&self, node: T, arguments: &[Value<W>]) -> Result<Outcome<W>, RunError>
    where
        T: AsRef<str>,
    {
        let Some((index, info)) = self.system.symbols.get(node.as_ref()) else {
            return Err(RunError::Unknown);
        };
        if info.kind != SymbolKind::Node {
            return Err(RunError::Kind(info.kind));
        }
        if arguments.len() != info.arity {
            return Err(RunError::Arity(ArityMismatch {
                expected: info.arity,
                received: arguments.len(),
            }));
        }
        Ok(self.run_raw(*index, arguments))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextMode {
    Active,
    Inactive,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RunError {
    Unknown,
    Arity(ArityMismatch),
    Kind(SymbolKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArityMismatch {
    pub expected: usize,
    pub received: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SystemSymbolError {
    Conflict {
        previous: SymbolInfo,
        current: SymbolInfo,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolInfo {
    pub kind: SymbolKind,
    pub source: SymbolSource,
    pub arity: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SymbolKind {
    Node,
    Action,
    Effect,
    Query,
    Getter,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SymbolSource {
    Api,
    File {
        path: Arc<Path>,
        line: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum SymbolSourceProto {
    Api,
    File {
        path: Arc<Path>,
    },
}

impl SymbolSourceProto {
    pub(crate) fn with_line(self, line: usize) -> SymbolSource {
        match self {
            Self::Api => SymbolSource::Api,
            Self::File { path } => SymbolSource::File { path, line },
        }
    }
}

