use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use smol_str::SmolStr;
use treelang::{Indent, Node as ScriptNode, ParseError, Offset, SourceContext, Tree, Span};
use walkdir::WalkDir;

use crate::gen::enum_class;
use crate::tree::ArityError;
use crate::tree::id_space::{IdSpace, Kind, NodeIdx, ActionIdx, IdError};
use crate::tree::script::ScriptError;

use super::{ScriptSource, ActionRoot, NodeRoot, ScriptResult};

use parse::*;
use produce::*;


mod parse;
mod produce;

type CompileResult<T = ()> = Result<T, CompileErrorKind>;

pub struct Compiler<'a, Ctx, Ext, Eff> {
    ids: IdSpace<Ctx, Ext, Eff>,
    indent: Indent,
    sources: Vec<Arc<str>>,
    contents: Vec<Cow<'a, str>>,
    action_root_placeholder: Arc<ActionRoot<Ext>>,
    node_root_placeholder: Arc<NodeRoot<Ext>>,
    declarations: HashMap<SmolStr, Registered>,
}

struct Registered {
    index: Root<NodeIdx, ActionIdx>,
    decl: Decl,
    src_index: usize,
}

impl<'a, Ctx, Ext, Eff> Compiler<'a, Ctx, Ext, Eff> {
    pub fn new(ids: IdSpace<Ctx, Ext, Eff>, indent: Indent) -> Self {
        Self {
            ids,
            indent,
            sources: Vec::new(),
            contents: Vec::new(),
            action_root_placeholder: Arc::default(),
            node_root_placeholder: Arc::default(),
            declarations: HashMap::new(),
        }
    }

    fn insert_node(&mut self, src_index: usize, node: ScriptNode) -> ScriptResult {
        let decl = parse_root_declaration(&node)
            .map_err(|kind| kind.into_script_error(self.source(src_index)))?;
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
            .map_err(|_| self.analyze_conflict(src_index, &decl))?;
        self.declarations.insert(name, Registered {
            index,
            decl: decl.into_inner(),
            src_index,
        });
        Ok(())
    }

    fn offset_context(&self, src_index: usize, offset: Offset) -> CompileContext {
        let src = self.source(src_index);
        CompileContext {
            source_name: src.name.into(),
            source_section: src.content.offset_section_display(offset).to_string().into(),
            source_line_number: offset.line_number(),
            source_column_number: src.content.byte_offset_on_line(offset) + 1,
        }
    }

    fn find_conflict_cause(&self, curr_src_index: usize, name: &SmolStr) -> ConflictErrorCause {
        let kind = self.ids.kind(name).unwrap();
        if let Some(decl) = self.declarations.get(name) {
            let context = self.offset_context(decl.src_index, decl.decl.node.location);
            if decl.src_index == curr_src_index {
                ConflictErrorCause::SameSource(kind, context)
            } else {
                ConflictErrorCause::DifferentSource(kind, context)
            }
        } else {
            ConflictErrorCause::Predefined(kind)
        }
    }

    fn analyze_conflict(&self, src_index: usize, decl: &Root<Decl>) -> ScriptError {
        ScriptError::Conflict(ConflictError {
            name: decl.name.value.to_smol_str(),
            span: decl.name.item.location,
            kind: decl.kind(),
            cause: self.find_conflict_cause(src_index, &decl.name.value),
            context: self.offset_context(src_index, decl.node.location),
        })
    }

    fn source(&self, src_index: usize) -> Source<'_> {
        Source {
            name: &self.sources[src_index],
            content: &self.contents[src_index],
        }
    }

    fn insert(&mut self, content: Cow<'a, str>, name: &str) -> ScriptResult {
        let src_index = self.sources.len();
        self.sources.push(name.into());
        self.contents.push(content);
        let src = self.source(src_index);
        let tree = Tree::parse(src.content, self.indent)
            .map_err(CompileErrorKind::Parse)
            .map_err(|kind| kind.into_script_error(src))?;
        for node in tree.roots {
            self.insert_node(src_index, node)?;
        }
        Ok(())
    }

    pub fn load(&mut self, source: ScriptSource<'a>) -> ScriptResult {
        match source {
            ScriptSource::Path { path } => {
                'entries: for entry in WalkDir::new(path) {
                    let entry = entry.map_err(|error| ScriptError::Find(Arc::new(error)))?;
                    let entry = entry.path();
                    if !entry.is_file() || !entry.ends_with(".rea") {
                        continue 'entries;
                    }
                    let content = std::fs::read_to_string(entry).map_err(|error| {
                        ScriptError::Read(entry.into(), Arc::new(error))
                    })?;
                    let entry = entry.to_string_lossy();
                    self.insert(Cow::Owned(content), &entry)?;
                }
                Ok(())
            },
            ScriptSource::Str { content, name } => {
                self.insert(Cow::Borrowed(content), &name)
            },
        }
    }

    pub fn compile(mut self) -> ScriptResult<IdSpace<Ctx, Ext, Eff>> {
        for (_, reg_decl) in std::mem::replace(&mut self.declarations, HashMap::default()) {
            let compiled = compile_root_declaration(&self.ids, &reg_decl.decl, reg_decl.index)
                .map_err(|kind| kind.into_script_error(self.source(reg_decl.src_index)))?;
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
    fn kind(&self) -> Kind {
        match self {
            Self::Node(_) => Kind::Node,
            Self::Action(_) => Kind::Action,
        }
    }

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

#[derive(Debug, Clone, thiserror::Error)]
#[error("{kind} at {}", context.position_display())]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub context: CompileContext,
}

#[derive(Debug, Clone, Copy)]
struct Source<'a> {
    content: &'a str,
    name: &'a str,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum CompileErrorKind {
    #[error(transparent)]
    Parse(ParseError),
    #[error("Wrong number of signature items for `{keyword}` directive: {error}")]
    DirectiveSignatureArity { keyword: &'static str, offset: Offset, error: ArityError },
    #[error("Wrong number of argument items for `{keyword}` directive: {error}")]
    DirectiveArgumentArity { keyword: &'static str, offset: Offset, error: ArityError },
    #[error("Wrong number of patterns for the given targets: {error}")]
    PatternArity { offset: Offset, error: ArityError },
    #[error("Invalid signature declaration")]
    InvalidRefDeclaration { offset: Offset },
    #[error("Invalid root declaration")]
    InvalidRootDeclaration { offset: Offset },
    #[error("Invalid query reference")]
    InvalidQueryRef { offset: Offset },
    #[error("Invalid effect reference")]
    InvalidEffectRef { offset: Offset },
    #[error("Variable `{name}` shadows existing lexical")]
    ShadowedLexical { name: SmolStr, span: Span },
    #[error("Variable `{name}` shadows existing global")]
    ShadowedGlobal { name: SmolStr, span: Span },
    #[error("Unbound variable `{name}`")]
    UnboundVariable { name: SmolStr, span: Span },
    #[error("`{name}`: {error}")]
    Identifier { name: SmolStr, span: Span, error: IdError },
    #[error("Unrecognized pattern")]
    UnrecognizedPattern { span: Span },
    #[error("Unrecognized value")]
    UnrecognizedValue { span: Span },
    #[error("Unrecognized node")]
    UnrecognizedNode { offset: Offset },
    #[error("Unrecognized action directive")]
    UnrecognizedActionDirective { offset: Offset },
}

impl CompileErrorKind {
    fn into_script_error(self, source: Source<'_>) -> ScriptError {
        ScriptError::Compile(self.into_compile_error(source))
    }

    fn into_compile_error(self, source: Source<'_>) -> CompileError {
        let (offset, context_section) = match self {
            | Self::Parse(ref error)
                => (error.offset(), error.section_display(source.content).to_string().into()),
            | Self::DirectiveSignatureArity { offset, .. }
            | Self::DirectiveArgumentArity { offset, .. }
            | Self::InvalidRefDeclaration { offset, .. }
            | Self::InvalidRootDeclaration { offset, .. }
            | Self::PatternArity { offset, .. }
            | Self::InvalidQueryRef { offset, .. }
            | Self::UnrecognizedNode { offset, .. }
            | Self::InvalidEffectRef { offset, .. }
            | Self::UnrecognizedActionDirective { offset, .. }
                => (offset, source.content.offset_section_display(offset).to_string().into()),
            | Self::ShadowedLexical { span, .. }
            | Self::ShadowedGlobal { span, .. }
            | Self::UnboundVariable { span, .. }
            | Self::Identifier { span, .. }
            | Self::UnrecognizedPattern { span, .. }
            | Self::UnrecognizedValue { span, .. }
                => (span.offset(), source.content.span_section_display(span).to_string().into()),
        };
        CompileError {
            kind: self,
            context: CompileContext {
                source_name: source.name.into(),
                source_section: context_section,
                source_line_number: offset.line_number(),
                source_column_number: source.content.byte_offset_on_line(offset),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompileContext {
    pub source_name: Arc<str>,
    pub source_section: Arc<str>,
    pub source_line_number: usize,
    pub source_column_number: usize,
}

impl std::fmt::Display for CompileContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "--> {}", self.position_display())?;
        self.source_section.fmt(f)
    }
}

impl CompileContext {
    fn position_display(&self) -> PositionDisplay<'_> {
        PositionDisplay { context: self }
    }
}

struct PositionDisplay<'a> {
    context: &'a CompileContext,
}

impl<'a> std::fmt::Display for PositionDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let CompileContext { source_name, source_line_number, source_column_number, .. }
            = &self.context;
        write!(f, "{}:{}:{}", source_name, source_line_number, source_column_number)
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error(
    "Conflicting definitions for `{name}` at {} ({kind}) and {cause}",
    context.position_display()
)]
pub struct ConflictError {
    pub name: SmolStr,
    pub span: Span,
    pub kind: Kind,
    pub cause: ConflictErrorCause,
    pub context: CompileContext,
}

#[derive(Debug, Clone)]
pub enum ConflictErrorCause {
    Predefined(Kind),
    SameSource(Kind, CompileContext),
    DifferentSource(Kind, CompileContext),
}

impl ConflictErrorCause {
    pub fn context(&self) -> Option<&CompileContext> {
        match self {
            Self::Predefined(_) => None,
            Self::SameSource(_, ctx) => Some(ctx),
            Self::DifferentSource(_, ctx) => Some(ctx),
        }
    }
}

impl std::fmt::Display for ConflictErrorCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Predefined(kind) =>
                write!(f, "a predefined identifier ({kind})"),
            Self::SameSource(kind, ctx) =>
                write!(f, "{} ({kind}) in the same source", ctx.position_display()),
            Self::DifferentSource(kind, ctx) =>
                write!(f, "{} ({kind})", ctx.position_display()),
        }
    }
}