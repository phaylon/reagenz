
use if_chain::if_chain;
use ramble::{Node, Item};
use smol_str::SmolStr;

use crate::World;
use crate::system::{
    System, NodeHook, SymbolKind, ArityMismatch, Outcome, Action, DispatchBuilderError,
};
use crate::value::{Value};

use super::parse::{
    match_group_directive, match_node_ref, match_variable, match_symbol, match_raw_ref,
    match_directive, match_free_directive, match_list,
};
use super::runtime::{NodeBranch, NodeValue, EffectRef, VarSpace, QuerySelection, QuerySource};
use super::{Declaration, CompileError, CompileErrorKind};
use super::kw;


type HookResult<W> = Result<Box<NodeHook<W>>, CompileError>;

pub(super) fn compile_declaration<W>(decl: Declaration<'_>, system: &System<W>) -> HookResult<W>
where
    W: World,
{
    match decl {
        Declaration::Node { name, parameters, node } => {
            compile_node(&Current { name, top: node, system }, parameters)
        },
        Declaration::Action { name, parameters, node } => {
            compile_action(&Current { name, top: node, system }, parameters)
        },
    }
}

#[derive(Clone, Copy)]
struct Current<'a, W: World> {
    top: &'a Node,
    name: &'a SmolStr,
    system: &'a System<W>,
}

fn compile_action<W>(curr: &Current<'_, W>, parameters: &[Item]) -> HookResult<W>
where
    W: World,
{
    let arity = parameters.len();
    let mut env = Env::new(curr.system);

    let (required, effects) = env.with(parameters, curr.top, |env| {
        let mut required = Vec::new();
        let mut effects = Vec::new();
        for node in &curr.top.nodes {
            if match_group_directive(node, kw::REQUIRED)? {
                required.extend(compile_node_branches(curr, env, &node.nodes)?);
            } else if match_group_directive(node, kw::EFFECTS)? {
                effects.extend(compile_effects(curr, env, &node.nodes)?);
            } else {
                return Err(CompileErrorKind::Unrecognized.at(node));
            }
        }
        Ok((
            NodeBranch::Sequence { branches: required },
            effects,
        ))
    })?;

    let name = curr.name.clone();
    let var_len = env.max_len;

    Ok(Box::new(move |ctx, arguments| {
        assert_eq!(arguments.len(), arity, "arity mismatch reached effect `{}`", name);
        if !ctx.is_active() {
            return Outcome::Failure;
        }
        let mut vars = VarSpace::with_capacity(var_len);
        vars.extend(arguments.iter().cloned());
        if !required.eval(&ctx.to_inactive(), &mut vars).is_success() {
            return Outcome::Failure;
        }
        let mut action = Action::default();
        for effect_ref in &effects {
            if let Some(effect) = effect_ref.eval(ctx, &mut vars) {
                action.effects.push(effect);
            } else {
                return Outcome::Failure;
            }
        }
        Outcome::Action(action)
    }))
}

fn compile_node<W>(curr: &Current<'_, W>, parameters: &[Item]) -> HookResult<W>
where
    W: World,
{
    let arity = parameters.len();
    let mut env = Env::new(curr.system);

    let logic = env.with(parameters, curr.top, |env| {
        let branches = compile_node_branches(curr, env, &curr.top.nodes)?;
        Ok(NodeBranch::Sequence { branches })
    })?;

    let name = curr.name.clone();
    let var_len = env.max_len;

    Ok(Box::new(move |ctx, arguments| {
        assert_eq!(arguments.len(), arity, "arity mismatch reached node `{}`", name);
        let mut vars = VarSpace::with_capacity(var_len);
        vars.extend(arguments.iter().cloned());
        logic.eval(ctx, &mut vars)
    }))
}

fn compile_effect<W>(
    curr: &Current<'_, W>,
    env: &mut Env<'_, W>,
    node: &Node,
) -> Result<EffectRef<W>, CompileError>
where
    W: World,
{
    if let Some((name, arguments)) = match_raw_ref(&node.items) {
        let arguments = compile_values(env, node, arguments)?;
        ensure_leaf_node(node)?;
        let accepted = &[SymbolKind::Effect];
        let effect = resolve_symbol(curr, node, name, accepted, arguments.len())?;
        Ok(EffectRef { effect, arguments })
    } else {
        Err(CompileErrorKind::InvalidEffectRefSyntax.at(node))
    }
}

fn compile_effects<'a, W>(
    curr: &Current<'_, W>,
    env: &mut Env<'_, W>,
    nodes: impl IntoIterator<Item = &'a Node>,
) -> Result<Vec<EffectRef<W>>, CompileError>
where
    W: World,
{
    let mut compiled = Vec::new();
    for node in nodes {
        compiled.push(compile_effect(curr, env, node)?);
    }
    Ok(compiled)
}

fn compile_node_branch<W>(
    curr: &Current<'_, W>,
    env: &mut Env<'_, W>,
    node: &Node,
) -> Result<NodeBranch<W>, CompileError>
where
    W: World,
{
    if match_group_directive(node, kw::SEQUENCE)? {
        let branches = compile_node_branches(curr, env, &node.nodes)?;
        Ok(NodeBranch::Sequence { branches })
    } else if match_group_directive(node, kw::SELECT)? {
        let branches = compile_node_branches(curr, env, &node.nodes)?;
        Ok(NodeBranch::Select { branches })
    } else if let Some((signature, arguments)) = match_directive(node, kw::QUERY)? {
        compile_query(curr, env, node, signature, arguments)
    } else if let Some((name, signature, arguments)) = match_free_directive(node) {
        let name = name.word().unwrap();
        let branches = compile_node_branches(curr, env, &node.nodes)?;
        let signature = compile_constant_values(signature)
            .ok_or_else(|| CompileErrorKind::InvalidDirectiveSyntax(name.as_str().into()).at(node))?;
        let callback = curr.system.dispatcher(name, signature).map_err(|error| match error {
            DispatchBuilderError::Unknown =>
                CompileErrorKind::UnknownDirective(name.clone()).at(node),
            DispatchBuilderError::Failed =>
                CompileErrorKind::InvalidDirectiveSyntax(name.clone()).at(node),
        })?;
        let arguments = compile_values(env, node, arguments)?;
        Ok(NodeBranch::Dispatcher { callback, arguments, branches })
    } else if let Some((name, mode, arguments)) = match_node_ref(&node.items) {
        let arguments = compile_values(env, node, arguments)?;
        ensure_leaf_node(node)?;
        let accepted = &[SymbolKind::Node, SymbolKind::Action];
        let node = resolve_symbol(curr, node, name, accepted, arguments.len())?;
        Ok(NodeBranch::Ref { node, arguments, mode })
    } else {
        Err(CompileErrorKind::Unrecognized.at(node))
    }
}

fn compile_query<W>(
    curr: &Current<'_, W>,
    env: &mut Env<'_, W>,
    node: &Node,
    signature: &[Item],
    items: &[Item],
) -> Result<NodeBranch<W>, CompileError>
where
    W: World,
{
    if_chain! {
        if signature.len() == 2;
        if let Some(selection) = signature[0].word_str().and_then(QuerySelection::from_str);
        if let Some(_) = match_variable(&signature[1]);
        if let Some((name, arguments)) = match_raw_ref(items);
        let arguments = compile_values(env, node, arguments)?;
        let accepted = &[SymbolKind::Query, SymbolKind::Getter];
        let index = resolve_symbol(curr, node, name, accepted, arguments.len())?;
        then {
            let branches = env.with(std::slice::from_ref(&signature[1]), node, |env| {
                compile_node_branches(curr, env, &node.nodes)
            })?;
            Ok(NodeBranch::Query {
                source: match curr.system.symbol(name.word_str().unwrap()).unwrap().kind {
                    SymbolKind::Query => QuerySource::Query(index),
                    SymbolKind::Getter => QuerySource::Getter(index),
                    _ => unreachable!(),
                },
                arguments,
                selection,
                branches,
            })
        } else {
            Err(CompileErrorKind::InvalidDirectiveSyntax(kw::QUERY.into()).at(node))
        }
    }
}

fn resolve_symbol<W>(
    curr: &Current<'_, W>,
    node: &Node,
    item: &Item,
    accepted: &[SymbolKind],
    arity: usize,
) -> Result<usize, CompileError>
where
    W: World,
{
    let name = item.word().unwrap();
    let span = item.inline_span;
    let Some(index) = curr.system.symbol_index(name) else {
        return Err(CompileErrorKind::UnknownSymbol(name.clone(), span).at(node));
    };
    let info = curr.system.symbol(name.as_str()).unwrap();
    if !accepted.contains(&info.kind) {
        return Err(CompileErrorKind::InvalidSymbolKind(name.clone(), span, info.kind).at(node));
    }
    if info.arity != arity {
        return Err(CompileErrorKind::InvalidSymbolArity(name.clone(), ArityMismatch {
            expected: info.arity,
            received: arity,
        }).at(node));
    }
    Ok(index)
}

fn ensure_leaf_node(node: &Node) -> Result<(), CompileError> {
    if node.nodes.is_empty() {
        Ok(())
    } else {
        Err(CompileErrorKind::UnexpectedSubTree.at(node))
    }
}

fn compile_node_branches<'a, W>(
    curr: &Current<'_, W>,
    env: &mut Env<'_, W>,
    nodes: impl IntoIterator<Item = &'a Node>,
) -> Result<Vec<NodeBranch<W>>, CompileError>
where
    W: World,
{
    let mut compiled = Vec::new();
    for node in nodes {
        compiled.push(compile_node_branch(curr, env, node)?);
    }
    Ok(compiled)
}

fn compile_constant_values<'a, W>(
    items: impl IntoIterator<Item = &'a Item>,
) -> Option<Vec<Value<W>>>
where
    W: World,
{
    let mut values = Vec::new();
    for item in items {
        values.push(Value::try_from_item(item)?);
    }
    Some(values)
}

fn compile_values<'a, W>(
    env: &mut Env<'_, W>,
    node: &Node,
    items: impl IntoIterator<Item = &'a Item>,
) -> Result<Vec<NodeValue<W>>, CompileError>
where
    W: World,
{
    let mut values = Vec::new();
    for item in items {
        if let Some(var) = match_variable(item) {
            values.push(env.find(var, item, node)?);
        } else if let Some(sym) = match_symbol(item) {
            values.push(NodeValue::Value(Value::Symbol(sym.clone())));
        } else if let Some(num) = item.num() {
            values.push(NodeValue::Value(match num {
                ramble::Num::Int(i) => Value::Int(i),
                ramble::Num::Float(f) => Value::Float(f),
            }));
        } else if let Some(items) = match_list(item) {
            values.push(NodeValue::List(compile_values(env, node, items)?));
        } else {
            return Err(CompileErrorKind::InvalidValue(item.inline_span).at(node));
        }
    }
    Ok(values)
}

struct Env<'a, W: World> {
    system: &'a System<W>,
    lexicals: Vec<SmolStr>,
    max_len: usize,
}

impl<'a, W> Env<'a, W>
where
    W: World,
{
    fn new(system: &'a System<W>) -> Self {
        Self {
            lexicals: Vec::new(),
            max_len: 0,
            system,
        }
    }

    fn find(
        &self,
        variable: &SmolStr,
        item: &Item,
        node: &Node,
    ) -> Result<NodeValue<W>, CompileError> {
        if let Some(index) = self.system.global_index(variable) {
            Ok(NodeValue::Global(index))
        } else if let Some(index) = self.lexicals.iter().position(|l| l == variable) {
            Ok(NodeValue::Lexical(index))
        } else {
            Err(CompileErrorKind::UnboundVariable(variable.clone(), item.inline_span).at(node))
        }
    }

    fn with<F, R>(
        &mut self,
        items: &[Item],
        node: &Node,
        body: F,
    ) -> Result<R, CompileError>
    where
        F: FnOnce(&mut Self) -> Result<R, CompileError>,
    {
        let len = self.lexicals.len();
        let mut env = scopeguard::guard(self, |env| {
            env.lexicals.truncate(len);
        });
        for item in items.iter().cloned() {
            let var = item.word().unwrap().clone();
            if env.lexicals.contains(&var) || env.system.global_index(&var).is_some() {
                return Err(CompileErrorKind::ShadowedVariable(var, item.inline_span).at(node));
            }
            env.lexicals.push(var);
        }
        env.max_len = env.max_len.max(env.lexicals.len());
        body(&mut env)
    }
}