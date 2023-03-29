use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use smol_str::SmolStr;
use treelang::{Indent, Node as ScriptNode, ParseError, Offset, SourceContext, Tree, Span};
use walkdir::WalkDir;

use crate::gen::enum_class;
use crate::tree::ArityError;
use crate::tree::id_map::Index;
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
    declarations: HashMap<SmolStr, Root<(Decl, Index, usize)>>,
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
        let prepared = (match decl {
            Root::Node(decl) => {
                let placeholder = self.node_root_placeholder.clone();
                match self.ids.set::<NodeIdx>(name.clone(), placeholder, arity) {
                    Ok(index) => Ok(Root::Node((decl, index.into(), src_index))),
                    Err(_) => Err(Root::Node(decl)),
                }
            },
            Root::Action(decl) => {
                let placeholder = self.action_root_placeholder.clone();
                match self.ids.set::<ActionIdx>(name.clone(), placeholder, arity) {
                    Ok(index) => Ok(Root::Action((decl, index.into(), src_index))),
                    Err(_) => Err(Root::Action(decl)),
                }
            },
        }).map_err(|decl| self.analyze_conflict(src_index, &decl))?;
        self.declarations.insert(name, prepared);
        Ok(())
    }

    fn offset_context(&self, src_index: usize, offset: Offset) -> CompileContext {
        let src = self.source(src_index);
        CompileContext {
            source_name: src.name.into(),
            source_section: src.content.offset_section_display(offset).to_string().into(),
        }
    }

    fn find_conflict_cause(&self, curr_src_index: usize, name: &SmolStr) -> ConflictErrorCause {
        let kind = self.ids.kind(name).unwrap();
        if let Some(decl) = self.declarations.get(name) {
            let orig_src_index = decl.2;
            let decl = &decl.0;
            let context = self.offset_context(orig_src_index, decl.node.location);
            if orig_src_index == curr_src_index {
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
        for (_, decl) in std::mem::replace(&mut self.declarations, HashMap::default()) {
            let (decl, index, src_index) = match decl {
                Root::Node((decl, index, src_index)) => (Root::Node(decl), index, src_index),
                Root::Action((decl, index, src_index)) => (Root::Action(decl), index, src_index),
            };
            let compiled = compile_root_declaration(&self.ids, decl.as_ref())
                .map_err(|kind| kind.into_script_error(self.source(src_index)))?;
            match compiled {
                Root::Node(root) => self.ids.set_node::<NodeIdx>(index.into(), Arc::new(root)),
                Root::Action(root) => self.ids.set_node::<ActionIdx>(index.into(), Arc::new(root)),
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

#[derive(Debug, Clone)]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub context: CompileContext,
}

#[derive(Debug, Clone, Copy)]
struct Source<'a> {
    content: &'a str,
    name: &'a str,
}

#[derive(Debug, Clone)]
pub enum CompileErrorKind {
    Parse(ParseError),
    DirectiveSignatureArity { keyword: &'static str, offset: Offset, error: ArityError },
    DirectiveArgumentArity { keyword: &'static str, offset: Offset, error: ArityError },
    PatternArity { offset: Offset, error: ArityError },
    InvalidRefDeclaration { offset: Offset },
    InvalidRootDeclaration { offset: Offset },
    InvalidQueryRef { offset: Offset },
    InvalidEffectRef { offset: Offset },
    ShadowedLexical { name: SmolStr, span: Span },
    ShadowedGlobal { name: SmolStr, span: Span },
    UnboundVariable { name: SmolStr, span: Span },
    Identifier { name: SmolStr, span: Span, error: IdError },
    UnrecognizedPattern { span: Span },
    UnrecognizedValue { span: Span },
    UnrecognizedNode { offset: Offset },
    UnrecognizedActionDirective { offset: Offset },
}

impl CompileErrorKind {
    fn into_script_error(self, source: Source<'_>) -> ScriptError {
        ScriptError::Compile(self.into_compile_error(source))
    }

    fn into_compile_error(self, source: Source<'_>) -> CompileError {
        let context_section = match self {
            | Self::Parse(ref error)
                => error.section_display(source.content).to_string().into(),
            | Self::DirectiveSignatureArity { offset, .. }
            | Self::DirectiveArgumentArity { offset, .. }
            | Self::InvalidRefDeclaration { offset, .. }
            | Self::InvalidRootDeclaration { offset, .. }
            | Self::PatternArity { offset, .. }
            | Self::InvalidQueryRef { offset, .. }
            | Self::UnrecognizedNode { offset, .. }
            | Self::InvalidEffectRef { offset, .. }
            | Self::UnrecognizedActionDirective { offset, .. }
                => source.content.offset_section_display(offset).to_string().into(),
            | Self::ShadowedLexical { span, .. }
            | Self::ShadowedGlobal { span, .. }
            | Self::UnboundVariable { span, .. }
            | Self::Identifier { span, .. }
            | Self::UnrecognizedPattern { span, .. }
            | Self::UnrecognizedValue { span, .. }
                => source.content.span_section_display(span).to_string().into(),
        };
        CompileError {
            kind: self,
            context: CompileContext {
                source_name: source.name.into(),
                source_section: context_section,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompileContext {
    pub source_name: Arc<str>,
    pub source_section: Arc<str>,
}

#[derive(Debug, Clone)]
pub struct ConflictError {
    pub name: SmolStr,
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