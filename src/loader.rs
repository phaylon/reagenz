
use ramble::{Tree, Node, NodeLocation, Item, Span};
use smol_str::SmolStr;

use crate::World;
use crate::system::{
    System, SymbolSourceProto, SystemSymbolError, SymbolSource, SymbolInfo, SymbolKind,
    ArityMismatch,
};

use self::parse::{require_ref_declaration, match_directive};
use self::compile::compile_declaration;

pub use self::runtime::Branch;


mod parse;
mod compile;
mod runtime;

pub(crate) mod kw;
pub(crate) mod mark;

pub(crate) fn load_str<W>(
    content: &str,
    mut system: System<W>,
    source: SymbolSourceProto,
) -> Result<System<W>, LoadError>
where
    W: World,
{
    let tree = Tree::parse(content, mark::MARKS).map_err(LoadError::Tree)?;

    let mut node_decls = Vec::new();
    for node in &tree.nodes {
        let decl = Declaration::extract(node)?;
        let source = source.clone().with_line(node.location.line_index + 1);
        let (name, info) = decl.symbol_info(source);
        system.register_node_raw(name.clone(), info, Box::new(|_, _| {
            panic!("loaded node was declared but not compiled in")
        })).map_err(|error| CompileErrorKind::SystemSymbol(error).at(node))?;
        node_decls.push((name, decl));
    }

    for (name, decl) in node_decls {
        let hooks = compile_declaration(decl, &system)?;
        system.replace_node_hook_raw(&name, hooks.node);
        if let Some(hook) = hooks.discovery {
            system.set_discovery_hook(name.clone(), hook);
        }
    }

    Ok(system)
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadError {
    Tree(ramble::ParseError),
    Compile(CompileError),
}

impl From<CompileError> for LoadError {
    fn from(error: CompileError) -> Self {
        LoadError::Compile(error)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub location: NodeLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompileErrorKind {
    Unrecognized,
    InvalidDirectiveForm,
    InvalidDeclaration,
    InvalidNodeDeclaration,
    SystemSymbol(SystemSymbolError),
    InvalidDirectiveSyntax(SmolStr),
    InvalidRefSyntax,
    InvalidEffectRefSyntax,
    ShadowedVariable(SmolStr, Span),
    UnboundVariable(SmolStr, Span),
    InvalidValue(Span),
    UnexpectedSubTree,
    UnknownSymbol(SmolStr, Span),
    InvalidSymbolKind(SmolStr, Span, SymbolKind),
    InvalidSymbolArity(SmolStr, ArityMismatch),
    UnknownDirective(SmolStr),
}

impl CompileErrorKind {
    fn at(self, node: &Node) -> CompileError {
        CompileError { kind: self, location: node.location.clone() }
    }
}

#[derive(Debug)]
enum Declaration<'a> {
    Node { name: &'a SmolStr, parameters: &'a [Item], node: &'a Node },
    Action { name: &'a SmolStr, parameters: &'a [Item], node: &'a Node },
}

impl<'a> Declaration<'a> {
    fn extract(node: &'a Node) -> Result<Declaration<'a>, LoadError> {
        let extract_node_declaration = |rest: &[Item], decl| {
            if !rest.is_empty() {
                Err(CompileErrorKind::InvalidDeclaration.at(node))
            } else {
                let (name, parameters) = require_ref_declaration(decl)
                    .map_err(|error| error.at(node))?;
                Ok((name, parameters))
            }
        };
        if let Some((rest, decl)) = match_directive(&node, kw::ACTION)? {
            let (name, parameters) = extract_node_declaration(rest, decl)?;
            Ok(Declaration::Action { name, parameters, node })
        } else if let Some((rest, decl)) = match_directive(&node, kw::NODE)? {
            let (name, parameters) = extract_node_declaration(rest, decl)?;
            Ok(Declaration::Node { name, parameters, node })
        } else {
            Err(CompileErrorKind::Unrecognized.at(node).into())
        }
    }

    fn symbol_info(&self, source: SymbolSource) -> (SmolStr, SymbolInfo) {
        let (name, parameters, kind) = match *self {
            Declaration::Node { name, parameters, .. } => {
                (name, parameters, SymbolKind::Node)
            },
            Declaration::Action { name, parameters, .. } => {
                (name, parameters, SymbolKind::Action)
            },
        };
        (name.clone(), SymbolInfo {
            source: source,
            arity: parameters.len(),
            kind,
        })
    }
}

pub(crate) fn is_reserved_char(c: char) -> bool {
    mark::MARKS.contains(&c)
    || ['$', '(', ')', '[', ']', '{', '}', ';'].contains(&c)
}
