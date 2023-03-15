use either::Either;
use smallvec::SmallVec;

use crate::World;
use crate::system::{Context, Outcome, ContextMode, Dispatcher};
use crate::value::{Value, Args};


pub(super) type VarSpace<W> = SmallVec<[Value<W>; 16]>;

pub(super) struct EffectRef<W: World> {
    pub effect: usize,
    pub arguments: Vec<NodeValue<W>>,
}

impl<W> EffectRef<W>
where
    W: World,
{
    pub(super) fn eval(&self, ctx: &Context<'_, W>, vars: &mut VarSpace<W>) -> Option<W::Effect> {
        let arguments: Args<_> = reify_values(ctx, &self.arguments, vars);
        ctx.effect_raw(self.effect, &arguments)
    }
}

pub(super) enum NodeBranch<W: World> {
    Select {
        branches: Vec<Self>,
    },
    Sequence {
        branches: Vec<Self>,
    },
    Complete {
        branches: Vec<Self>,
    },
    None {
        branches: Vec<Self>,
    },
    Ref {
        node: usize,
        arguments: Vec<NodeValue<W>>,
        mode: ContextMode,
    },
    Query {
        source: QuerySource,
        arguments: Vec<NodeValue<W>>,
        selection: QuerySelection,
        branches: Vec<Self>,
    },
    Match {
        value: NodeValue<W>,
        items: Vec<MatchItem<W>>,
        branches: Vec<Self>,
    },
    Dispatcher {
        callback: Box<Dispatcher<W>>,
        arguments: Vec<NodeValue<W>>,
        branches: Vec<Self>,
    },
}

impl<W> NodeBranch<W>
where
    W: World,
{
    pub(super) fn eval(&self, ctx: &Context<'_, W>, vars: &mut VarSpace<W>) -> Outcome<W> {
        match self {
            Self::Select { branches } => {
                eval_selection(ctx, vars, branches)
            },
            Self::Sequence { branches } => {
                eval_sequence(ctx, vars, branches)
            },
            Self::None { branches } => {
                eval_none(ctx, vars, branches)
            },
            Self::Complete { branches } => {
                for branch in branches {
                    branch.eval(ctx, vars);
                }
                Outcome::Success
            },
            Self::Ref { node, arguments, mode } => {
                let arguments: Args<_> = reify_values(ctx, arguments, vars);
                if mode.is_active() {
                    ctx.run_raw(*node, &arguments)
                } else {
                    ctx.to_inactive().run_raw(*node, &arguments)
                }
            },
            Self::Dispatcher { callback, arguments, branches } => {
                let arguments = reify_values(ctx, arguments, vars);
                let branches = branches.iter().map(|branch| Branch { vars, branch }).collect();
                callback(ctx, arguments, branches)
            },
            Self::Match { value, items, branches } => 'scope: {
                let value = reify_value(ctx, value, vars);
                let Value::List(values) = value else {
                    break 'scope Outcome::Failure;
                };
                if values.len() != items.len() {
                    break 'scope Outcome::Failure;
                }
                let vars_len = vars.len();
                let mut vars = scopeguard::guard(vars, |vars| {
                    vars.truncate(vars_len);
                });
                for (item, value) in items.iter().zip(values.iter()) {
                    match item {
                        MatchItem::Exact(exact) => {
                            let exact = reify_value(ctx, exact, &mut vars);
                            if *value != exact {
                                break 'scope Outcome::Failure;
                            }
                        },
                        MatchItem::BindLexical(_) => {
                            vars.push(value.clone());
                        }
                    }
                }
                eval_sequence(ctx, &mut vars, branches)
            },
            Self::Query { source, arguments, selection, branches } => {
                let arguments: Args<_> = reify_values(ctx, arguments, vars);
                let iter = match source {
                    QuerySource::Getter(index) => {
                        Either::Left(ctx.get_raw(*index, &arguments).into_iter())
                    },
                    QuerySource::Query(index) => {
                        Either::Right(ctx.query_raw(*index, &arguments))
                    },
                };
                let vars_len = vars.len();
                let mut vars = scopeguard::guard(vars, |vars| {
                    vars.truncate(vars_len);
                });
                match selection {
                    QuerySelection::Complete => {
                        for item in iter {
                            vars.push(item);
                            eval_sequence(ctx, &mut vars, branches);
                            vars.pop();
                        }
                        Outcome::Success
                    },
                    QuerySelection::Any => 'selection: {
                        for item in iter {
                            vars.push(item);
                            let result = eval_sequence(ctx, &mut vars, branches);
                            if result.is_non_failure() {
                                break 'selection result;
                            }
                            vars.pop();
                        }
                        Outcome::Failure
                    },
                    QuerySelection::All => 'selection: {
                        for item in iter {
                            vars.push(item);
                            let result = eval_sequence(ctx, &mut vars, branches);
                            if result.is_non_success() {
                                break 'selection result;
                            }
                            vars.pop();
                        }
                        Outcome::Success
                    },
                    QuerySelection::First => {
                        let mut iter = iter;
                        if let Some(item) = iter.next() {
                            vars.push(item);
                            let result = eval_sequence(ctx, &mut vars, branches);
                            vars.pop();
                            result
                        } else {
                            Outcome::Failure
                        }
                    },
                    QuerySelection::Last => {
                        if let Some(item) = iter.last() {
                            vars.push(item);
                            let result = eval_sequence(ctx, &mut vars, branches);
                            vars.pop();
                            result
                        } else {
                            Outcome::Failure
                        }
                    },
                }
            },
        }
    }
}

pub struct Branch<'a, W: World> {
    vars: &'a VarSpace<W>,
    branch: &'a NodeBranch<W>,
}

impl<'a, W> Branch<'a, W>
where
    W: World,
{
    pub fn eval(&self, ctx: &Context<'_, W>) -> Outcome<W> {
        let mut vars = self.vars.clone();
        self.branch.eval(ctx, &mut vars)
    }
}

fn eval_selection<W>(
    ctx: &Context<'_, W>,
    vars: &mut VarSpace<W>,
    branches: &[NodeBranch<W>],
) -> Outcome<W>
where
    W: World,
{
    for branch in branches {
        let result = branch.eval(ctx, vars);
        if result.is_non_failure() {
            return result;
        }
    }
    Outcome::Failure
}

fn eval_none<W>(
    ctx: &Context<'_, W>,
    vars: &mut VarSpace<W>,
    branches: &[NodeBranch<W>],
) -> Outcome<W>
where
    W: World,
{
    let ctx = ctx.to_inactive();
    for branch in branches {
        let result = branch.eval(&ctx, vars);
        if result.is_success() {
            return Outcome::Failure;
        }
    }
    Outcome::Success
}

fn eval_sequence<W>(
    ctx: &Context<'_, W>,
    vars: &mut VarSpace<W>,
    branches: &[NodeBranch<W>],
) -> Outcome<W>
where
    W: World,
{
    for branch in branches {
        let result = branch.eval(ctx, vars);
        if result.is_non_success() {
            return result;
        }
    }
    Outcome::Success
}
fn reify_value<W>(
    ctx: &Context<'_, W>,
    value: &NodeValue<W>,
    vars: &[Value<W>],
) -> Value<W>
where
    W: World,
{
    match value {
        NodeValue::Value(value) => value.clone(),
        NodeValue::Lexical(index) => vars[*index].clone(),
        NodeValue::Global(index) => ctx.global_raw(*index),
        NodeValue::List(values) => Value::List(reify_values(ctx, values, vars)),
    }
}

fn reify_values<W, C>(
    ctx: &Context<'_, W>,
    values: &[NodeValue<W>],
    vars: &[Value<W>],
) -> C
where
    W: World,
    C: FromIterator<Value<W>>,
{
    values.iter().map(|value| reify_value(ctx, value, vars)).collect()
}

pub(super) enum NodeValue<W: World> {
    Value(Value<W>),
    Lexical(usize),
    Global(usize),
    List(Vec<Self>),
}

impl<W> NodeValue<W>
where
    W: World,
{
    pub(super) fn lexical(&self) -> Option<usize> {
        if let Self::Lexical(index) = *self {
            Some(index)
        } else {
            None
        }
    }
}

pub(super) enum MatchItem<W: World> {
    Exact(NodeValue<W>),
    BindLexical(usize),
}

pub(super) enum QuerySource {
    Getter(usize),
    Query(usize),
}

pub(super) enum QuerySelection {
    Any,
    All,
    First,
    Last,
    Complete,
}

impl QuerySelection {
    pub(super) fn from_str(value: &str) -> Option<Self> {
        match value {
            "any" => Some(Self::Any),
            "all" => Some(Self::All),
            "first" => Some(Self::First),
            "last" => Some(Self::Last),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }
}