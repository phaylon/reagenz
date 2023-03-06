use std::collections::HashMap;

use derivative::Derivative;

use crate::{World, Value, Symbol, ValueIter};


type SymbolMap = HashMap<Symbol, (usize, SymbolInfo)>;
type NodeHook<W> = dyn Fn(&Context<'_, W>, &[Value<W>]) -> Outcome<W>;
type EffectHook<W> = dyn Fn(&Context<'_, W>, &[Value<W>]) -> <W as World>::Effect;
type QueryHook<W> = dyn Fn(&Context<'_, W>, &[Value<W>]) -> Box<ValueIter<W>>;
type GetterHook<W> = dyn Fn(&Context<'_, W>, &[Value<W>]) -> Option<Value<W>>;

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

#[derive(Derivative)]
#[derivative(
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

    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> + '_ {
        self.symbols.keys()
    }

    pub fn symbol<T>(&self, value: T) -> Option<&SymbolInfo>
    where
        T: AsRef<str>,
    {
        self.symbols.get(value.as_ref()).map(|(_, info)| info)
    }

    pub fn register_effect<S, F, const N: usize>(
        &mut self,
        name: S,
        body: F,
    ) -> Result<(), SystemSymbolError>
    where
        S: Into<Symbol>,
        F: Fn(&Context<'_, W>, &[Value<W>; N]) -> W::Effect + 'static,
    {
        register_symbol(
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
        S: Into<Symbol>,
        F: Fn(&Context<'_, W>, &[Value<W>; N]) -> Box<ValueIter<W>> + 'static,
    {
        register_symbol(
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
        S: Into<Symbol>,
        F: Fn(&Context<'_, W>, &[Value<W>; N]) -> Option<Value<W>> + 'static,
    {
        register_symbol(
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
        S: Into<Symbol>,
        F: Fn(&Context<'_, W>, &[Value<W>; N]) -> Outcome<W> + 'static,
    {
        register_symbol(
            &mut self.symbols,
            &mut self.nodes,
            SymbolSource::Api,
            name.into(),
            SymbolKind::Node,
            body,
        )
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

fn register_symbol<W, R, F, const N: usize>(
    symbols: &mut SymbolMap,
    hooks: &mut Vec<Box<dyn Fn(&Context<'_, W>, &[Value<W>]) -> R>>,
    source: SymbolSource,
    name: Symbol,
    kind: SymbolKind,
    body: F,
) -> Result<(), SystemSymbolError>
where
    W: World,
    F: Fn(&Context<'_, W>, &[Value<W>; N]) -> R + 'static,
{
    let info = SymbolInfo {
        kind,
        source,
        arity: N,
    };

    if let Some((_, previous)) = symbols.get(&name).cloned() {
        Err(SystemSymbolError::Conflict { previous, current: info })
    } else {
        let index = hooks.len();
        hooks.push({
            let name = name.clone();
            Box::new(move |ctx, arguments| {
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
        });
        symbols.insert(name, (index, info));
        Ok(())
    }
}

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

    pub fn run<T>(
        &self,
        node: T,
        arguments: &[Value<W>],
    ) -> Result<Outcome<W>, RunError>
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
        Ok(self.system.nodes[*index](self, arguments))
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
    expected: usize,
    received: usize,
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
    Effect,
    Query,
    Getter,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SymbolSource {
    Api,
}

