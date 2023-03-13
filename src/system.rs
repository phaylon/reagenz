use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use derivative::Derivative;
use smol_str::SmolStr;
use walkdir::WalkDir;

use crate::World;
use crate::core::load_core_system;
use crate::loader::kw::DIRECTIVES;
use crate::loader::{LoadError, load_str, Branch};
use crate::value::{Value, ValueIter, StrExt, Args};


type SymbolMap = HashMap<SmolStr, (usize, SymbolInfo)>;
type VariableMap = HashMap<SmolStr, usize>;

pub type Dispatcher<W> = dyn Fn(
    &Context<'_, W>,
    Args<Value<W>>,
    Args<Branch<'_, W>>,
) -> Outcome<W>;

pub(crate) type DispatchBuilder<W> = dyn Fn(
    &System<W>,
    Vec<Value<W>>,
) -> Option<Box<Dispatcher<W>>>;

pub(crate) type DispatchBuilderMap<W> = HashMap<SmolStr, Box<DispatchBuilder<W>>>;

pub(crate) type NodeHook<W> = dyn Fn(&Context<'_, W>, &[Value<W>]) -> Outcome<W>;
pub(crate) type EffectHook<W> = dyn Fn(&Context<'_, W>, &[Value<W>]) -> Option<<W as World>::Effect>;
pub(crate) type QueryHook<W> = dyn Fn(&Context<'_, W>, &[Value<W>]) -> Box<ValueIter<W>>;
pub(crate) type GetterHook<W> = dyn Fn(&Context<'_, W>, &[Value<W>]) -> Option<Value<W>>;
pub(crate) type GlobalHook<W> = dyn Fn(&Context<'_, W>) -> Value<W>;

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
    pub fn from_effect(effect: W::Effect) -> Self {
        Self::from(Vec::from([effect]))
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failure)
    }

    pub fn is_non_success(&self) -> bool {
        !self.is_success()
    }

    pub fn is_non_failure(&self) -> bool {
        !self.is_failure()
    }

    pub fn action(&self) -> Option<&Action<W>> {
        if let Self::Action(action) = self {
            Some(action)
        } else {
            None
        }
    }

    pub fn effects(&self) -> Option<&[W::Effect]> {
        self.action().map(|a| a.effects.as_slice())
    }
}

impl<W> From<Vec<W::Effect>> for Outcome<W>
where
    W: World,
{
    fn from(effects: Vec<W::Effect>) -> Self {
        Outcome::Action(Action { effects })
    }
}

impl<W> From<Action<W>> for Outcome<W>
where
    W: World,
{
    fn from(action: Action<W>) -> Self {
        Outcome::Action(action)
    }
}

impl<W> From<bool> for Outcome<W>
where
    W: World,
{
    fn from(is_true: bool) -> Self {
        if is_true {
            Self::Success
        } else {
            Self::Failure
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
    global_variables: VariableMap,
    dispatch_builders: DispatchBuilderMap<W>,
    nodes: Vec<Box<NodeHook<W>>>,
    effects: Vec<Box<EffectHook<W>>>,
    queries: Vec<Box<QueryHook<W>>>,
    getters: Vec<Box<GetterHook<W>>>,
    globals: Vec<Box<GlobalHook<W>>>,
}

impl<W> System<W>
where
    W: World,
{
    pub fn new() -> Self {
        Self {
            symbols: SymbolMap::new(),
            global_variables: VariableMap::new(),
            dispatch_builders: DispatchBuilderMap::new(),
            nodes: Vec::new(),
            effects: Vec::new(),
            queries: Vec::new(),
            getters: Vec::new(),
            globals: Vec::new(),
        }
    }

    pub fn core() -> Self {
        load_core_system(Self::new()).unwrap()
    }

    pub fn context<'a>(&'a self, state: &'a W::State) -> Context<'a, W> {
        Context::new(state, self)
    }

    pub fn load_from_str(self, content: &str) -> Result<Self, LoadError> {
        load_str(content, self, SymbolSourceProto::Api)
    }

    pub fn load_from_file<P>(self, path: P) -> Result<Self, FileLoadError>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|error| {
            FileLoadError {
                path: path.into(),
                kind: FileLoadErrorKind::Read(Arc::new(error)),
            }
        })?;
        load_str(&content, self, SymbolSourceProto::File { path: path.into() }).map_err(|error| {
            FileLoadError {
                path: path.into(),
                kind: FileLoadErrorKind::Load(error),
            }
        })
    }

    pub fn load_from_directory<P>(mut self, path: P) -> Result<Self, FileLoadError>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        'entries: for entry in WalkDir::new(path) {
            let entry = entry.map_err(|error| {
                FileLoadError {
                    path: path.into(),
                    kind: FileLoadErrorKind::Walk(Arc::new(error)),
                }
            })?;
            if !entry.file_name().to_str().map_or(false, |f| f.ends_with(".rea")) {
                continue 'entries;
            }
            self = self.load_from_file(entry.path())?;
        }
        Ok(self)
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

    pub fn globals(&self) -> impl Iterator<Item = &SmolStr> + '_ {
        self.global_variables.keys()
    }

    pub(crate) fn global_index(&self, value: &str) -> Option<usize> {
        self.global_variables.get(value).copied()
    }

    pub fn register_global<S, F>(&mut self, name: S, body: F) -> Result<(), SystemGlobalError>
    where
        S: Into<SmolStr>,
        F: Fn(&Context<'_, W>) -> Value<W> + 'static,
    {
        let name = name.into();
        if self.global_variables.contains_key(&name) {
            return Err(SystemGlobalError::Conflict);
        }
        if !name.is_variable() {
            return Err(SystemGlobalError::Invalid);
        }
        let index = self.globals.len();
        self.globals.push(Box::new(body));
        self.global_variables.insert(name, index);
        Ok(())
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

    pub(crate) fn dispatcher(
        &self,
        name: &str,
        signature: Vec<Value<W>>,
    ) -> Result<Box<Dispatcher<W>>, DispatchBuilderError> {
        if let Some(builder) = self.dispatch_builders.get(name) {
            if let Some(dispatcher) = builder(self, signature) {
                Ok(dispatcher)
            } else {
                Err(DispatchBuilderError::Failed)
            }
        } else {
            Err(DispatchBuilderError::Unknown)
        }
    }

    pub fn register_dispatch<S, F>(
        &mut self,
        name: S,
        body: F,
    ) -> Result<(), SystemDispatcherError>
    where
        S: Into<SmolStr>,
        F: Fn(&System<W>, Vec<Value<W>>) -> Option<Box<Dispatcher<W>>> + 'static,
    {
        let name = name.into();
        if !name.is_symbol() {
            return Err(SystemDispatcherError::Invalid);
        }
        if DIRECTIVES.contains(&name.as_str()) {
            return Err(SystemDispatcherError::BuiltinConflict);
        }
        if self.dispatch_builders.contains_key(&name) {
            return Err(SystemDispatcherError::DispatcherConflict);
        }
        self.dispatch_builders.insert(name, Box::new(body));
        Ok(())
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

pub(crate) enum DispatchBuilderError {
    Unknown,
    Failed,
}

#[derive(Debug, Clone)]
pub struct FileLoadError {
    pub kind: FileLoadErrorKind,
    pub path: Arc<Path>,
}

#[derive(Debug, Clone)]
pub enum FileLoadErrorKind {
    Walk(Arc<walkdir::Error>),
    Read(Arc<std::io::Error>),
    Load(LoadError),
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
    if !name.is_symbol() {
        Err(SystemSymbolError::Invalid)
    } else if let Some((_, previous)) = symbols.get(&name).cloned() {
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
    state: &'a W::State,
    system: &'a System<W>,
    mode: ContextMode,
}

impl<'a, W> Context<'a, W>
where
    W: World,
{
    pub fn new(state: &'a W::State, system: &'a System<W>) -> Self {
        Self { state, system, mode: ContextMode::Active }
    }

    pub fn system(&self) -> &'a System<W> {
        self.system
    }

    pub fn state(&self) -> &'a W::State {
        self.state
    }

    pub fn is_active(&self) -> bool {
        self.mode == ContextMode::Active
    }

    pub fn to_inactive(&self) -> Self {
        Self {
            state: self.state,
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

    pub(crate) fn global_raw(&self, index: usize) -> Value<W> {
        self.system.globals[index](self)
    }

    fn resolve_symbol(
        &self,
        name: &str,
        arity: usize,
        accepted: &[SymbolKind],
    ) -> Result<(usize, SymbolKind), RunError> {
        let Some((index, info)) = self.system.symbols.get(name) else {
            return Err(RunError::Unknown);
        };
        if !accepted.contains(&info.kind) {
            return Err(RunError::Kind(info.kind));
        }
        if arity != info.arity {
            return Err(RunError::Arity(ArityMismatch {
                expected: info.arity,
                received: arity,
            }));
        }
        Ok((*index, info.kind))
    }

    pub fn global(&self, name: &str) -> Option<Value<W>> {
        let index = self.system.global_variables.get(name)?;
        Some(self.global_raw(*index))
    }

    pub fn effect(&self, name: &str, arguments: &[Value<W>]) -> Result<Option<W::Effect>, RunError> {
        let accepted = &[SymbolKind::Effect];
        let (index, _) = self.resolve_symbol(name.as_ref(), arguments.len(), accepted)?;
        Ok(self.effect_raw(index, arguments))
    }

    pub fn run(&self, name: &str, arguments: &[Value<W>]) -> Result<Outcome<W>, RunError> {
        let accepted = &[SymbolKind::Node, SymbolKind::Action];
        let (index, _) = self.resolve_symbol(name.as_ref(), arguments.len(), accepted)?;
        Ok(self.run_raw(index, arguments))
    }

    pub fn query(&self, name: &str, arguments: &[Value<W>]) -> Result<Box<ValueIter<W>>, RunError> {
        let accepted = &[SymbolKind::Query, SymbolKind::Getter];
        let (index, kind) = self.resolve_symbol(name.as_ref(), arguments.len(), accepted)?;
        match kind {
            SymbolKind::Query => Ok(self.query_raw(index, arguments)),
            SymbolKind::Getter => Ok(Box::new(self.get_raw(index, arguments).into_iter())),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextMode {
    Active,
    Inactive,
}

impl ContextMode {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
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
    Invalid,
    Conflict {
        previous: SymbolInfo,
        current: SymbolInfo,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SystemDispatcherError {
    Invalid,
    BuiltinConflict,
    DispatcherConflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SystemGlobalError {
    Invalid,
    Conflict,
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

