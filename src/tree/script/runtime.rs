use std::borrow::Cow;
use std::sync::Arc;

use fastrand::Rng;
use smallvec::SmallVec;

use crate::tree::{RefIdx, SeedIdx};
use crate::{Outcome, Action};
use crate::tree::context::Context;
use crate::tree::id_space::{EffectIdx, GlobalIdx, QueryIdx, ActionIdx, NodeIdx};
use crate::value::Value;


pub type Nodes<Ext> = Arc<[Node<Ext>]>;
pub type ProtoValues<Ext> = Arc<[ProtoValue<Ext>]>;

pub type Patterns<Ext> = Arc<[Pattern<Ext>]>;

type Lex<Ext> = SmallVec<[Value<Ext>; 8]>;
type Args<Ext> = SmallVec<[Value<Ext>; 4]>;

type Seeds = Arc<[SeedIdx]>;

#[derive(Debug, Clone)]
pub struct ActionRoot<Ext> {
    pub index: Option<ActionIdx>,
    pub effects: Arc<[(EffectIdx, ProtoValues<Ext>)]>,
    pub conditions: Nodes<Ext>,
    pub discovery: Nodes<Ext>,
    pub lexicals: usize,
}

impl<Ext> ActionRoot<Ext>
where
    Ext: Clone + PartialEq,
{
    pub fn eval_discovery_nodes<C, Ctx, Eff>(&self, ctx: &C)
    where
        C: Context<Ctx, Ext, Eff>,
    {
        let mut lex = Lex::with_capacity(self.lexicals);
        for node in self.discovery.iter() {
            node.eval(ctx, &mut lex);
        }
    }

    pub fn eval<C, Ctx, Eff>(
        &self,
        ctx: &C,
        arguments: &[Value<Ext>],
    ) -> Outcome<Ext, Eff>
    where
        C: Context<Ctx, Ext, Eff>,
    {
        let mut lex = Lex::with_capacity(self.lexicals);
        lex.extend(arguments.iter().cloned());
        if !self.conditions_ok(ctx, &mut lex) {
            return Outcome::Failure;
        }
        let mut effects = SmallVec::<[Eff; 16]>::with_capacity(self.effects.len());
        for (index, arguments) in self.effects.iter() {
            let arguments: Args<Ext> = reify_values(ctx, &mut lex, arguments.iter());
            if let Some(effect) = ctx.tree().ids.get(*index)(ctx.view(), &arguments) {
                effects.push(effect);
            } else {
                return Outcome::Failure;
            }
        }
        ctx.action(Action::new(
            self.index.unwrap(),
            arguments.into(),
            effects.into_iter().collect(),
        ))
    }

    fn conditions_ok<C, Ctx, Eff>(
        &self,
        ctx: &C,
        lex: &mut Lex<Ext>,
    ) -> bool
    where
        C: Context<Ctx, Ext, Eff>,
    {
        let ctx = ctx.to_inactive_if_active();
        eval_sequence(ctx.as_ref(), lex, &self.conditions).is_success()
    }
}

impl<Ext> Default for ActionRoot<Ext> {
    fn default() -> Self {
        Self {
            index: None,
            effects: Arc::new([]),
            conditions: Arc::new([]),
            discovery: Arc::new([]),
            lexicals: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeRoot<Ext> {
    pub index: Option<NodeIdx>,
    pub node: Node<Ext>,
    pub lexicals: usize,
}

impl<Ext> NodeRoot<Ext>
where
    Ext: Clone + PartialEq,
{
    pub fn eval<C, Ctx, Eff>(
        &self,
        ctx: &C,
        arguments: &[Value<Ext>],
    ) -> Outcome<Ext, Eff>
    where
        C: Context<Ctx, Ext, Eff>,
    {
        let mut lex = Lex::with_capacity(self.lexicals);
        lex.extend(arguments.iter().cloned());
        self.node.eval(ctx, &mut lex)
    }
}

impl<Ext> Default for NodeRoot<Ext> {
    fn default() -> Self {
        Self {
            index: None,
            node: Node::Failure,
            lexicals: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ProtoValue<Ext> {
    Global(GlobalIdx),
    Lexical(usize),
    Value(Value<Ext>),
    List(ProtoValues<Ext>),
}

impl<Ext> ProtoValue<Ext> {
    fn reify<C, Ctx, Eff>(&self, ctx: &C, lex: &mut Lex<Ext>) -> Value<Ext>
    where
        C: Context<Ctx, Ext, Eff>,
        Ext: Clone,
    {
        match self {
            Self::Global(index) => ctx.tree().ids.get(*index)(ctx.view()),
            Self::Lexical(index) => lex[*index].clone(),
            Self::Value(value) => value.clone(),
            Self::List(values) => Value::List(reify_values(ctx, lex, values.iter())),
        }
    }
}

fn reify_values<'i, R, C, Ctx, Ext, Eff>(
    ctx: &C,
    lex: &mut Lex<Ext>,
    values: impl IntoIterator<Item = &'i ProtoValue<Ext>>,
) -> R
where
    C: Context<Ctx, Ext, Eff>,
    R: FromIterator<Value<Ext>>,
    Ext: Clone + 'i,
{
    values.into_iter().map(|pv| pv.reify(ctx, lex)).collect()
}

#[derive(Debug, Clone)]
pub enum Node<Ext> {
    Failure,
    Dispatch(Dispatch, Nodes<Ext>),
    Ref(RefIdx, RefMode, ProtoValues<Ext>),
    Query(Pattern<Ext>, QueryIdx, ProtoValues<Ext>, QueryMode, Nodes<Ext>),
    Match(ProtoValues<Ext>, Patterns<Ext>, Nodes<Ext>),
    Random(u64, Seeds, Nodes<Ext>),
}

impl<Ext> Node<Ext> {
    fn eval<C, Ctx, Eff>(&self, ctx: &C, lex: &mut Lex<Ext>) -> Outcome<Ext, Eff>
    where
        C: Context<Ctx, Ext, Eff>,
        Ext: Clone + PartialEq,
    {
        match self {
            Self::Failure => Outcome::Failure,
            Self::Dispatch(dispatch, branches) => {
                dispatch.eval_branches(ctx, lex, branches)
            },
            Self::Ref(ref_kind, mode, arguments) => {
                let arguments: Args<Ext> = reify_values(ctx, lex, arguments.iter());
                ref_kind.eval(ctx, *mode, &arguments)
            },
            Self::Match(values, patterns, branches) => {
                let values: Args<Ext> = reify_values(ctx, lex, values.iter());
                let lex_len = lex.len();
                let mut lex = scopeguard::guard(lex, |lex| lex.truncate(lex_len));
                let is_matched = patterns.iter()
                    .zip(values.iter())
                    .all(|(p, v)| p.try_apply(ctx, &mut lex, v));
                if is_matched {
                    eval_sequence(ctx, &mut lex, branches)
                } else {
                    Outcome::Failure
                }
            },
            Self::Query(pattern, index, arguments, mode, branches) => {
                let arguments: Args<Ext> = reify_values(ctx, lex, arguments.iter());
                mode.eval_query(ctx, lex, *index, &arguments, pattern, branches)
            },
            Self::Random(seed, ctx_seeds, branches) => {
                let mut branches: SmallVec::<[_; 16]> = branches.iter().cloned().collect();
                let mut seed = *seed;
                for ctx_seed in ctx_seeds.iter() {
                    let ctx_seed = ctx.tree().ids.get(*ctx_seed)(ctx.view());
                    seed = seed.wrapping_add(ctx_seed);
                }
                let rng = Rng::with_seed(seed);
                rng.shuffle(&mut branches);
                eval_selection(ctx, lex, &branches)
            },
        }
    }

    pub fn sequence(nodes: Nodes<Ext>) -> Self {
        Self::Dispatch(Dispatch::Sequence, nodes)
    }
}

impl RefIdx {
    fn eval<C, Ctx, Ext, Eff>(
        &self,
        ctx: &C,
        mode: RefMode,
        arguments: &[Value<Ext>],
    ) -> Outcome<Ext, Eff>
    where
        C: Context<Ctx, Ext, Eff>,
        Ext: Clone + PartialEq,
    {
        let ctx = mode.apply(ctx);
        match self {
            Self::Action(index) => {
                ctx.tree().ids.get(*index).eval(ctx.as_ref(), arguments)
            },
            Self::Cond(index) => {
                ctx.tree().ids.get(*index)(ctx.view(), arguments).into()
            },
            Self::Node(index) => {
                ctx.tree().ids.get(*index).eval(ctx.as_ref(), arguments)
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefMode {
    Query,
    Inherit,
}

impl RefMode {
    fn apply<'c, C, Ctx, Ext, Eff>(&self, ctx: &'c C) -> Cow<'c, C>
    where
        C: Context<Ctx, Ext, Eff>,
    {
        match self {
            Self::Query => ctx.to_inactive_if_active(),
            Self::Inherit => Cow::Borrowed(ctx),
        }
    }
}

fn eval_sequence<C, Ctx, Ext, Eff>(
    ctx: &C,
    lex: &mut Lex<Ext>,
    nodes: &[Node<Ext>],
) -> Outcome<Ext, Eff>
where
    C: Context<Ctx, Ext, Eff>,
    Ext: Clone + PartialEq,
{
    Dispatch::Sequence.eval_branches(ctx, lex, nodes)
}

fn eval_selection<C, Ctx, Ext, Eff>(
    ctx: &C,
    lex: &mut Lex<Ext>,
    nodes: &[Node<Ext>],
) -> Outcome<Ext, Eff>
where
    C: Context<Ctx, Ext, Eff>,
    Ext: Clone + PartialEq,
{
    Dispatch::Selection.eval_branches(ctx, lex, nodes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    Sequence,
    Selection,
    None,
    Visit,
}

impl Dispatch {
    fn eval_branches<C, Ctx, Ext, Eff>(
        &self,
        ctx: &C,
        lex: &mut Lex<Ext>,
        nodes: &[Node<Ext>],
    ) -> Outcome<Ext, Eff>
    where
        C: Context<Ctx, Ext, Eff>,
        Ext: Clone + PartialEq,
    {
        match self {
            Dispatch::Sequence => 'eval: {
                for node in nodes {
                    let result = node.eval(ctx, lex);
                    if result.is_non_success() {
                        break 'eval result;
                    }
                }
                Outcome::Success
            },
            Dispatch::Selection => 'eval: {
                for node in nodes {
                    let result = node.eval(ctx, lex);
                    if result.is_non_failure() {
                        break 'eval result;
                    }
                }
                Outcome::Failure
            },
            Dispatch::None => 'eval: {
                for node in nodes {
                    let result = node.eval(ctx, lex);
                    if result.is_non_failure() {
                        break 'eval Outcome::Failure;
                    }
                }
                Outcome::Success
            },
            Dispatch::Visit => {
                for node in nodes {
                    node.eval(ctx, lex);
                }
                Outcome::Success
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    Sequence,
    Selection,
    First,
    Last,
    Visit,
}

impl QueryMode {
    fn eval_query<C, Ctx, Ext, Eff>(
        &self,
        ctx: &C,
        lex: &mut Lex<Ext>,
        index: QueryIdx,
        arguments: &[Value<Ext>],
        pattern: &Pattern<Ext>,
        branches: &Nodes<Ext>,
    ) -> Outcome<Ext, Eff>
    where
        C: Context<Ctx, Ext, Eff>,
        Ext: Clone + PartialEq,
    {
        let lex_len = lex.len();
        let mut lex = scopeguard::guard(lex, move |lex| lex.truncate(lex_len));
        match self {
            Self::Sequence => {
                let query_fn = ctx.tree().ids.get(index);
                query_fn(ctx.view(), arguments, &mut |iter| {
                    'values: for topic_value in iter {
                        lex.truncate(lex_len);
                        if !pattern.try_apply(ctx, &mut lex, &topic_value) {
                            continue 'values;
                        }
                        let result = eval_sequence(ctx, &mut lex, branches);
                        if result.is_non_success() {
                            return result;
                        }
                    }
                    Outcome::Success
                })
            },
            Self::Selection => {
                let query_fn = ctx.tree().ids.get(index);
                query_fn(ctx.view(), arguments, &mut |iter| {
                    'values: for topic_value in iter {
                        lex.truncate(lex_len);
                        if !pattern.try_apply(ctx, &mut lex, &topic_value) {
                            continue 'values;
                        }
                        let result = eval_sequence(ctx, &mut lex, branches);
                        if result.is_non_failure() {
                            return result;
                        }
                    }
                    Outcome::Failure
                })
            },
            Self::First => {
                let query_fn = ctx.tree().ids.get(index);
                query_fn(ctx.view(), arguments, &mut |iter| {
                    let Some(topic_value) = iter.next() else {
                        return Outcome::Failure;
                    };
                    if !pattern.try_apply(ctx, &mut lex, &topic_value) {
                        return Outcome::Failure;
                    }
                    eval_sequence(ctx, &mut lex, branches)
                })
            },
            Self::Last => {
                let query_fn = ctx.tree().ids.get(index);
                query_fn(ctx.view(), arguments, &mut |iter| {
                    let Some(topic_value) = iter.last() else {
                        return Outcome::Failure;
                    };
                    if !pattern.try_apply(ctx, &mut lex, &topic_value) {
                        return Outcome::Failure;
                    }
                    eval_sequence(ctx, &mut lex, branches)
                })
            },
            Self::Visit => {
                let query_fn = ctx.tree().ids.get(index);
                query_fn(ctx.view(), arguments, &mut |iter| {
                    'values: for topic_value in iter {
                        lex.truncate(lex_len);
                        if !pattern.try_apply(ctx, &mut lex, &topic_value) {
                            continue 'values;
                        }
                        eval_sequence(ctx, &mut lex, branches);
                    }
                    Outcome::Success
                })
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum Pattern<Ext> {
    Exact(Value<Ext>),
    Bind,
    Lexical(usize),
    Global(GlobalIdx),
    List(Patterns<Ext>),
}

impl<Ext> Pattern<Ext> {
    pub fn try_apply<C, Ctx, Eff>(
        &self,
        ctx: &C,
        lex: &mut Lex<Ext>,
        value: &Value<Ext>,
    ) -> bool
    where
        C: Context<Ctx, Ext, Eff>,
        Ext: Clone + PartialEq,
    {
        match self {
            Self::Bind => {
                lex.push(value.clone());
                true
            },
            Self::Exact(exact) => value == exact,
            Self::Lexical(index) => *value == lex[*index],
            Self::Global(index) => *value == ctx.tree().ids.get(*index)(ctx.view()),
            Self::List(patterns) => {
                if let Value::List(values) = value {
                    patterns.len() == values.len() && patterns
                        .iter()
                        .zip(values.iter())
                        .all(|(p, v)| p.try_apply(ctx, lex, v))
                } else {
                    false
                }
            },
        }
    }
}