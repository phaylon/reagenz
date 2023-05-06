use std::sync::Arc;

use ordered_float::OrderedFloat;
use src_ctx::SourceError;
use treelang::{Node as ScriptNode, Item, ItemKind};

use crate::tree::{ArityError, ActionIdx, NodeIdx, RefIdx};
use crate::tree::id_space::{IdSpace, IdError, EffectIdx};
use crate::tree::script::{
    NodeRoot, ActionRoot, Node, Nodes, Dispatch, RefMode, Patterns, Pattern, ProtoValues,
    ProtoValue, QueryMode,
};
use crate::value::Value;

use super::parse::{
    Var, ItemValue, kw, try_parse_label_directive, match_ref, Sym, match_var, match_sym,
    match_directive, try_parse_keyword_directive, match_wildcard,
};
use super::{Root, Decl, ScriptResult, ScriptError, RefClass};

use env::*;


mod env;

pub(super) fn compile_root_declaration<Ctx, Ext, Eff>(
    ids: &IdSpace<Ctx, Ext, Eff>,
    decl: &Decl,
    index: Root<NodeIdx, ActionIdx>,
) -> ScriptResult<Root<NodeRoot<Ext>, ActionRoot<Ext>>> {
    index.map_each(
        |index| {
            compile_node_root(index, ids, &decl.parameters, decl.node.children())
        },
        |index| {
            compile_action_root(index, ids, &decl.parameters, decl.node.children())
        },
    ).lift().map_err(|error| error.with_context(decl.node.location))
}

fn compile_node_root<Ctx, Ext, Eff>(
    index: NodeIdx,
    ids: &IdSpace<Ctx, Ext, Eff>,
    parameters: &[ItemValue<Var>],
    children: &[ScriptNode],
) -> ScriptResult<NodeRoot<Ext>> {
    let mut env = Env::new(ids);
    env.scope(parameters.iter(), |env| {
        let nodes = compile_branches(env, children)?;
        let lexicals = env.max_vars();
        Ok(NodeRoot { index: Some(index), node: Node::sequence(nodes), lexicals })
    })
}

fn compile_action_root<Ctx, Ext, Eff>(
    index: ActionIdx,
    ids: &IdSpace<Ctx, Ext, Eff>,
    parameters: &[ItemValue<Var>],
    children: &[ScriptNode],
) -> ScriptResult<ActionRoot<Ext>> {
    let mut conditions = Vec::new();
    let mut effects = Vec::new();
    let mut discovery = Vec::new();

    'children: for child in children {
        for (keyword, collection) in [
            (kw::def::action::CONDITIONS, &mut conditions),
            (kw::def::action::EFFECTS, &mut effects),
            (kw::def::action::DISCOVERY, &mut discovery),
        ] {
            if try_parse_label_directive(child, keyword)? {
                collection.extend(child.children().iter().cloned());
                continue 'children;
            }
        }
        return Err(SourceError::new(
            ScriptError::UnrecognizedActionDirective,
            child.location,
            "expected action directive",
        ));
    }

    let mut env = Env::new(ids);
    let discovery = compile_branches(&mut env, &discovery)?;

    env.scope(parameters.iter(), |env| {
        let conditions = compile_branches(env, &conditions)?;
        let effects = compile_effects(env, &effects)?;
        let lexicals = env.max_vars();
        Ok(ActionRoot { index: Some(index), effects, conditions, discovery, lexicals })
    })
}

fn compile_effects<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    nodes: &[ScriptNode],
) -> ScriptResult<Arc<[(EffectIdx, ProtoValues<Ext>)]>> {
    let mut compiled = Vec::new();
    for node in nodes {
        compiled.push(compile_effect(env, node)?);
    }
    Ok(compiled.into())
}

fn compile_effect<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    node: &ScriptNode,
) -> ScriptResult<(EffectIdx, ProtoValues<Ext>)> {
    let (name, arguments) = node.statement()
        .and_then(|stmt| match_ref(&stmt.signature))
        .filter(|(name, _)| matches!(name, RefClass::Raw(_)))
        .ok_or(SourceError::new(
            ScriptError::InvalidEffectRef,
            node.location,
            "expected effect reference",
        ))?;
    let index = env.ids().resolve(&name, arguments.len())
        .map_err(|error| convert_id_error(&name, error))?;
    let arguments = compile_values(env, arguments)?;
    Ok((index, arguments))
}

fn compile_branches<'i, Ctx, Ext, Eff, I>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    nodes: I,
) -> ScriptResult<Nodes<Ext>>
where
    I: IntoIterator<Item = &'i ScriptNode>,
{
    let mut compiled = Vec::new();
    for node in nodes {
        compiled.push(compile_branch(env, node)?);
    }
    Ok(compiled.into())
}

fn try_compile_branch_random<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    node: &ScriptNode,
) -> ScriptResult<Option<Node<Ext>>> {
    let (seeds, any) = if let Some(seeds) = try_parse_keyword_directive(node, kw::dir::RANDOM)? {
        (seeds, false)
    } else if let Some(seeds) = try_parse_keyword_directive(node, kw::dir::RANDOM_ANY)? {
        (seeds, true)
    } else {
        return Ok(None);
    };

    let mut ctx_seeds = Vec::new();
    for seed in seeds {
        let Some(name) = match_sym(seed) else {
            return Err(SourceError::new(
                ScriptError::InvalidSeedRef,
                seed.location.start(),
                "expected seed reference",
            ));
        };
        let index = env.ids().resolve(name.as_str(), 0)
            .map_err(|error| convert_id_error(&name, error))?;
        ctx_seeds.push(index);
    }
    let branches = compile_branches(env, node.children())?;
    Ok(Some(Node::Random(fastrand::u64(..), ctx_seeds.into(), branches, any)))
}

fn try_compile_branch_dispatch<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    node: &ScriptNode,
) -> ScriptResult<Option<Node<Ext>>> {
    for (keyword, mode) in [
        (kw::dir::SEQUENCE, Dispatch::Sequence),
        (kw::dir::SELECT, Dispatch::Selection),
        (kw::dir::NONE, Dispatch::None),
        (kw::dir::VISIT, Dispatch::Visit),
    ] {
        if try_parse_label_directive(node, keyword)? {
            return Ok(Some(Node::Dispatch(mode, compile_branches(env, node.children())?)));
        }
    }
    Ok(None)
}

fn convert_id_error(
    name: &ItemValue<Sym>,
    error: IdError,
) -> SourceError<ScriptError> {
    SourceError::new(
        ScriptError::Identifier { name: name.to_smol_str(), error },
        name.item.location.start(),
        "identifier",
    )
}

fn resolve_ref_symbol<Ctx, Ext, Eff>(
    env: &Env<'_, Ctx, Ext, Eff>,
    name: &ItemValue<Sym>,
    arity: usize,
) -> ScriptResult<RefIdx> {
    env.ids().resolve_ref(name.as_str(), arity).map_err(|error| convert_id_error(name, error))
}

fn try_compile_branch_ref<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    node: &ScriptNode,
) -> ScriptResult<Option<Node<Ext>>> {
    if let Some(stmt) = node.statement() {
        if let Some((ref_name, arguments)) = match_ref(&stmt.signature) {
            let (value, mode) = match ref_name {
                RefClass::Query(value) => (value, RefMode::Query),
                RefClass::Raw(value) => (value, RefMode::Inherit),
            };
            let node_ref = resolve_ref_symbol(env, &value, arguments.len())?;
            let arguments = compile_values(env, arguments)?;
            return Ok(Some(Node::Ref(node_ref, mode, arguments)));
        }
    }
    Ok(None)
}

fn try_compile_branch_switch<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    node: &ScriptNode,
) -> ScriptResult<Option<Node<Ext>>> {
    if let Some(targets) = try_parse_keyword_directive(node, kw::dir::switch::SWITCH)? {
        let mut cases = Vec::new();
        for child in node.children() {
            if let Some(patterns) = try_parse_keyword_directive(child, kw::dir::switch::CASE)? {
                if targets.len() != patterns.len() {
                    return Err(SourceError::new(
                        ScriptError::PatternArity {
                            error: ArityError { expected: targets.len(), given: patterns.len() },
                        },
                        child.location,
                        "switch case with arity mismatch",
                    ));
                }
                env.scope([], |env| {
                    let targets = compile_values(env, targets)?;
                    let patterns = compile_pattern_items(env, patterns)?;
                    let branches = compile_branches(env, child.children())?;
                    cases.push(Node::Match(targets, patterns, branches));
                    Ok(())
                })?;
            } else {
                return Err(SourceError::new(
                    ScriptError::InvalidSwitchCase,
                    child.location,
                    "expected switch case node",
                ));
            }
        }
        return Ok(Some(Node::Dispatch(Dispatch::Selection, cases.into())));
    }
    Ok(None)
}

fn try_compile_branch_match<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    node: &ScriptNode,
) -> ScriptResult<Option<Node<Ext>>> {
    if let Some((patterns, targets)) = match_directive(node, kw::dir::MATCH) {
        if targets.len() != patterns.len() {
            return Err(SourceError::new(
                ScriptError::PatternArity {
                    error: ArityError { expected: targets.len(), given: patterns.len() },
                },
                node.location,
                "match with arity mismatch",
            ));
        }
        return env.scope([], |env| {
            let targets = compile_values(env, targets)?;
            let patterns = compile_pattern_items(env, patterns)?;
            let branches = compile_branches(env, node.children())?;
            Ok(Some(Node::Match(targets, patterns, branches)))
        });
    }
    Ok(None)
}

fn try_compile_branch_query<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    node: &ScriptNode,
) -> ScriptResult<Option<Node<Ext>>> {
    for (keyword, mode) in [
        (kw::dir::query::SELECT, QueryMode::Selection),
        (kw::dir::query::SEQUENCE, QueryMode::Sequence),
        (kw::dir::query::FIRST, QueryMode::First),
        (kw::dir::query::LAST, QueryMode::Last),
        (kw::dir::query::VISIT, QueryMode::Visit),
    ] {
        if let Some((signature, arguments)) = match_directive(node, keyword) {
            let [pattern] = signature else {
                return Err(SourceError::new(
                    ScriptError::DirectiveSignatureArity {
                        keyword,
                        error: ArityError { expected: 1, given: signature.len() },
                    },
                    node.location,
                    "query with invalid signature",
                ));
            };
            let Some((RefClass::Raw(name), arguments)) = match_ref(arguments) else {
                return Err(SourceError::new(
                    ScriptError::InvalidQueryRef,
                    node.location,
                    "expected query reference",
                ));
            };
            let index = env.ids().resolve(&name, arguments.len())
                .map_err(|error| convert_id_error(&name, error))?;
            return env.scope([], |env| {
                let arguments = compile_values(env, arguments)?;
                let pattern = compile_pattern_item(env, pattern)?;
                let branches = compile_branches(env, node.children())?;
                Ok(Some(Node::Query(pattern, index, arguments, mode, branches)))
            });
        }
    }
    Ok(None)
}

fn compile_branch<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    node: &ScriptNode,
) -> ScriptResult<Node<Ext>> {
    if let Some(compiled) = try_compile_branch_dispatch(env, node)? {
        Ok(compiled)
    } else if let Some(compiled) = try_compile_branch_ref(env, node)? {
        Ok(compiled)
    } else if let Some(compiled) = try_compile_branch_match(env, node)? {
        Ok(compiled)
    } else if let Some(compiled) = try_compile_branch_switch(env, node)? {
        Ok(compiled)
    } else if let Some(compiled) = try_compile_branch_query(env, node)? {
        Ok(compiled)
    } else if let Some(compiled) = try_compile_branch_random(env, node)? {
        Ok(compiled)
    } else {
        Err(SourceError::new(ScriptError::UnrecognizedNode, node.location, "expected logic node"))
    }
}

fn compile_value<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    item: &Item,
) -> ScriptResult<ProtoValue<Ext>> {
    if let Some(var) = match_var(item) {
        env.resolve(&var)
    } else if let Some(sym) = match_sym(item) {
        Ok(ProtoValue::Value(sym.to_smol_str().into()))
    } else if let ItemKind::Int(value) = item.kind {
        Ok(ProtoValue::Value(Value::Int(value)))
    } else if let ItemKind::Float(value) = item.kind {
        Ok(ProtoValue::Value(Value::Float(OrderedFloat(value))))
    } else if let ItemKind::Brackets(values) = &item.kind {
        Ok(ProtoValue::List(compile_values(env, values)?))
    } else {
        Err(SourceError::new(
            ScriptError::UnrecognizedValue,
            item.location.start(),
            "expected value",
        ))
    }
}

fn compile_values<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    values: &[Item],
) -> ScriptResult<ProtoValues<Ext>> {
    let mut compiled = Vec::new();
    for value in values {
        compiled.push(compile_value(env, value)?);
    }
    Ok(compiled.into())
}

fn compile_pattern_item<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    item: &Item,
) -> ScriptResult<Pattern<Ext>> {
    if match_wildcard(item) {
        Ok(Pattern::Ignore)
    } else if let Some(var) = match_var(item) {
        Ok(env.resolve_pattern(&var))
    } else if let Some(sym) = match_sym(item) {
        Ok(Pattern::Exact(sym.to_smol_str().into()))
    } else if let ItemKind::Int(value) = item.kind {
        Ok(Pattern::Exact(Value::Int(value)))
    } else if let ItemKind::Float(value) = item.kind {
        Ok(Pattern::Exact(Value::Float(OrderedFloat(value))))
    } else if let ItemKind::Brackets(items) = &item.kind {
        Ok(Pattern::List(compile_pattern_items(env, items)?))
    } else {
        Err(SourceError::new(
            ScriptError::UnrecognizedPattern,
            item.location.start(),
            "expected pattern",
        ))
    }
}

fn compile_pattern_items<Ctx, Ext, Eff>(
    env: &mut Env<'_, Ctx, Ext, Eff>,
    items: &[Item],
) -> ScriptResult<Patterns<Ext>> {
    let mut compiled = Vec::new();
    for item in items {
        compiled.push(compile_pattern_item(env, item)?);
    }
    Ok(compiled.into())
}
