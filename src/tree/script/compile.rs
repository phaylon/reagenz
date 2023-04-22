use std::collections::HashMap;
use std::sync::Arc;

use smol_str::SmolStr;
use src_ctx::{SourceMap, LoadError, ContextError, SourceError, SourceIndex, Origin};
use treelang::{Indent, Node as ScriptNode, ParseError, Tree};

use crate::gen::enum_class;
use crate::tree::ArityError;
use crate::tree::id_space::{IdSpace, NodeIdx, ActionIdx, IdError};

use super::{ScriptSource, ActionRoot, NodeRoot};

use parse::*;
use produce::*;


mod parse;
mod produce;

pub(crate) type CompileResult<T = ()> = Result<T, CompileError>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum CompileError {
    #[error(transparent)]
    Load(#[from] LoadError),
    #[error(transparent)]
    Script(#[from] ContextError<ScriptError>),
    #[error(transparent)]
    Conflict(#[from] ContextError<ConflictError>),
    #[error("Multiple definitions of named source `{name}`")]
    NamedSourceConflict { name: Arc<str> },
}

impl CompileError {
    pub fn display_with_context(&self) -> impl std::fmt::Display + '_ {
        struct FullDisplay<'a>(&'a CompileError);
        impl<'a> std::fmt::Display for FullDisplay<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match &self.0 {
                    CompileError::Load(error) => writeln!(f, "error: {error}"),
                    CompileError::Script(error) => error.display_with_context().fmt(f),
                    CompileError::Conflict(error) => error.display_with_context().fmt(f),
                    CompileError::NamedSourceConflict { .. } => writeln!(f, "error: {self}"),
                }
            }
        }
        FullDisplay(self)
    }
}

pub type ScriptResult<T = ()> = Result<T, SourceError<ScriptError>>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum ScriptError {
    #[error(transparent)]
    Parse(ParseError),
    #[error("Wrong number of signature items for `{keyword}` directive: {error}")]
    DirectiveSignatureArity { keyword: &'static str, error: ArityError },
    #[error("Wrong number of argument items for `{keyword}` directive: {error}")]
    DirectiveArgumentArity { keyword: &'static str, error: ArityError },
    #[error("Wrong number of patterns for the given targets: {error}")]
    PatternArity { error: ArityError },
    #[error("Invalid signature declaration")]
    InvalidRefDeclaration,
    #[error("Invalid root declaration")]
    InvalidRootDeclaration,
    #[error("Invalid query reference")]
    InvalidQueryRef,
    #[error("Invalid effect reference")]
    InvalidEffectRef,
    #[error("Invalid seed reference")]
    InvalidSeedRef,
    #[error("Invalid switch case node")]
    InvalidSwitchCase,
    #[error("Variable `{name}` shadows existing lexical")]
    ShadowedLexical { name: SmolStr },
    #[error("Variable `{name}` shadows existing global")]
    ShadowedGlobal { name: SmolStr },
    #[error("Unbound variable `{name}`")]
    UnboundVariable { name: SmolStr },
    #[error("for `{name}`: {error}")]
    Identifier { name: SmolStr, error: IdError },
    #[error("Unrecognized pattern")]
    UnrecognizedPattern,
    #[error("Unrecognized value")]
    UnrecognizedValue,
    #[error("Unrecognized node")]
    UnrecognizedNode,
    #[error("Unrecognized action directive")]
    UnrecognizedActionDirective,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Conflict with {} definition of `{symbol}`", self.kind())]
pub struct ConflictError {
    pub symbol: SmolStr,
    pub is_internal: bool,
}

impl ConflictError {
    fn kind(&self) -> &str {
        if self.is_internal { "internal" } else { "user" }
    }
}

pub struct Compiler<Ctx, Ext, Eff> {
    ids: IdSpace<Ctx, Ext, Eff>,
    indent: Indent,
    sources: SourceMap,
    action_root_placeholder: Arc<ActionRoot<Ext>>,
    node_root_placeholder: Arc<NodeRoot<Ext>>,
    declarations: HashMap<SmolStr, Registered>,
}

struct Registered {
    index: Root<NodeIdx, ActionIdx>,
    decl: Decl,
}

impl<Ctx, Ext, Eff> Compiler<Ctx, Ext, Eff> {
    pub fn new(ids: IdSpace<Ctx, Ext, Eff>, indent: Indent) -> Self {
        Self {
            ids,
            indent,
            sources: SourceMap::new(),
            action_root_placeholder: Arc::default(),
            node_root_placeholder: Arc::default(),
            declarations: HashMap::new(),
        }
    }

    fn insert_node(&mut self, node: ScriptNode) -> CompileResult {
        let decl = parse_root_declaration(&node)
            .map_err(|error| error.into_context_error(&self.sources))?;
        let name = decl.name.value.to_smol_str();
        let arity = decl.parameters.len();
        let index = decl.as_ref()
            .map_node(|_| {
                let placeholder = self.node_root_placeholder.clone();
                self.ids.set::<NodeIdx>(name.clone(), placeholder, arity)
            })
            .map_action(|_| {
                let placeholder = self.action_root_placeholder.clone();
                self.ids.set::<ActionIdx>(name.clone(), placeholder, arity)
            })
            .lift()
            .map_err(|_| self.analyze_conflict(&decl))?;
        self.declarations.insert(name, Registered {
            index,
            decl: decl.into_inner(),
        });
        Ok(())
    }

    fn analyze_conflict(&self, decl: &Root<Decl>) -> CompileError {
        let name = decl.name.to_smol_str();
        let prev = self.declarations.get(&name);
        let error = ConflictError { symbol: name, is_internal: prev.is_none() };
        let mut origins = Vec::new();
        origins.push(self.sources.context_error_origin(
            decl.node.location,
            "second definition",
            None,
        ));
        if let Some(prev) = prev {
            origins.insert(0, self.sources.context_error_origin(
                prev.decl.node.location,
                "first definition",
                None,
            ));
        }
        CompileError::Conflict(ContextError::with_origins(error, origins))
    }

    fn parse(&mut self, index: SourceIndex) -> CompileResult {
        let input = self.sources.input(index);
        let tree = Tree::parse(input, self.indent)
            .map_err(|error| error.map(ScriptError::Parse).into_context_error(&self.sources))?;
        for node in tree.roots {
            self.insert_node(node)?;
        }
        Ok(())
    }

    pub fn load(&mut self, source: ScriptSource) -> CompileResult {
        match source {
            ScriptSource::Path { path } => {
                let inserted = self.sources.load_directory(path, ".rea")?
                    .into_iter()
                    .filter_map(|insert| insert.try_into_inserted().ok());
                for index in inserted {
                    self.parse(index)?;
                }
                Ok(())
            },
            ScriptSource::Str { content, name } => {
                let index = self.sources.insert(Origin::Named(name.clone()), content)
                    .try_into_inserted().ok()
                    .ok_or_else(|| CompileError::NamedSourceConflict { name })?;
                self.parse(index)
            },
        }
    }

    pub fn compile(mut self) -> CompileResult<IdSpace<Ctx, Ext, Eff>> {
        for (_, reg_decl) in std::mem::replace(&mut self.declarations, HashMap::default()) {
            let compiled = compile_root_declaration(&self.ids, &reg_decl.decl, reg_decl.index)
                .map_err(|error| error.into_context_error(&self.sources))?;
            match compiled {
                Root::Node(root) => self.ids.set_node(root.index.unwrap(), Arc::new(root)),
                Root::Action(root) => self.ids.set_node(root.index.unwrap(), Arc::new(root)),
            }
        }
        Ok(self.ids)
    }
}

struct Decl {
    name: ItemValue<Sym>,
    parameters: Vec<ItemValue<Var>>,
    node: ScriptNode,
}

enum_class!(Root {
    Node = (),
    Action = Node,
});

impl<Node, Action> Root<Node, Action> {
    fn map_node<F, T>(self, mapv: F) -> Root<T, Action>
    where
        F: FnOnce(Node) -> T,
    {
        match self {
            Self::Node(node) => Root::Node(mapv(node)),
            Self::Action(action) => Root::Action(action),
        }
    }

    fn map_action<F, T>(self, mapv: F) -> Root<Node, T>
    where
        F: FnOnce(Action) -> T,
    {
        match self {
            Self::Node(node) => Root::Node(node),
            Self::Action(action) => Root::Action(mapv(action)),
        }
    }

    fn map_each<FN, RN, FA, RA>(self, mapn: FN, mapa: FA) -> Root<RN, RA>
    where
        FN: FnOnce(Node) -> RN,
        FA: FnOnce(Action) -> RA,
    {
        match self {
            Root::Node(n) => Root::Node(mapn(n)),
            Root::Action(a) => Root::Action(mapa(a)),
        }
    }
}

enum_class!(RefClass {
    Raw = (),
    Query = Raw,
});
