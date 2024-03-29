use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::Arc;

use fastrand::Rng;
use log::trace;
use smallvec::SmallVec;

use crate::tree::{RefIdx, SeedIdx, External, Effect};
use crate::{Outcome, Action};
use crate::tree::context::{Context, DiscoveryContext};
use crate::tree::id_space::{EffectIdx, GlobalIdx, QueryIdx, ActionIdx, NodeIdx};
use crate::value::Value;


pub type Nodes<Ext> = Arc<[Node<Ext>]>;
pub type ProtoValues<Ext> = Arc<[ProtoValue<Ext>]>;

pub type Patterns<Ext> = Arc<[Pattern<Ext>]>;

pub type CondBranches<Ext> = Arc<[(Node<Ext>, Node<Ext>)]>;
pub type CondElseBranch<Ext> = Arc<Node<Ext>>;

type Lex<Ext> = SmallVec<[Value<Ext>; 8]>;
type Args<Ext> = SmallVec<[Value<Ext>; 4]>;

type Seeds = Arc<[SeedIdx]>;

#[derive(Debug, Clone)]
pub struct ActionRoot<Ext> {
    pub index: Option<ActionIdx>,
    pub effects: Arc<[(EffectIdx, ProtoValues<Ext>)]>,
    pub inherit: Nodes<Ext>,
    //pub inherit_required: Arc<[(ActionIdx, ProtoValues<Ext>)]>,
    //pub inherit_optional: Arc<[(ActionIdx, ProtoValues<Ext>)]>,
    pub conditions: Nodes<Ext>,
    pub discovery: Nodes<Ext>,
    pub lexicals: usize,
}

impl<Ext> ActionRoot<Ext>
where
    Ext: External,
{
    pub fn eval_discovery_nodes<C, Ctx, Eff>(&self, ctx: &C)
    where
        C: Context<Ctx, Ext, Eff>,
        Eff: Effect,
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
        Eff: Effect,
    {
        let mut lex = Lex::with_capacity(self.lexicals);
        lex.extend(arguments.iter().cloned());
        if !self.conditions_ok(ctx, &mut lex) {
            return Outcome::Failure;
        }
        let mut effects = SmallVec::<[Eff; 32]>::with_capacity(self.effects.len());
        for (index, arguments) in self.effects.iter() {
            let arguments: Args<Ext> = reify_values(ctx, &mut lex, arguments.iter());
            if let Some(effect) = ctx.tree().ids.get(*index)(ctx.view(), &arguments) {
                effects.push(effect);
            } else {
                return Outcome::Failure;
            }
        }
        let mut inherited = Vec::new();
        let collection = RefCell::new(&mut inherited);
        let discovery_ctx = DiscoveryContext::from_context(ctx, &collection, None);
        for node in self.inherit.iter() {
            let result = node.eval(&discovery_ctx, &mut lex);
            if result.is_failure() {
                return Outcome::Failure;
            }
        }
        for action in inherited {
            effects.extend(action.effects().iter().cloned());
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
        Eff: Effect,
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
            inherit: Arc::new([]),
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
    Ext: External,
{
    pub fn eval<C, Ctx, Eff>(
        &self,
        ctx: &C,
        arguments: &[Value<Ext>],
    ) -> Outcome<Ext, Eff>
    where
        C: Context<Ctx, Ext, Eff>,
        Eff: Effect,
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
    Success,
    Failure,
    Dispatch(Dispatch, Nodes<Ext>),
    Ref(RefIdx, RefMode, ProtoValues<Ext>),
    Query(Pattern<Ext>, QueryIdx, ProtoValues<Ext>, QueryMode, Nodes<Ext>),
    Match(ProtoValues<Ext>, Patterns<Ext>, Nodes<Ext>),
    Random(u64, Seeds, Nodes<Ext>, bool),
    Cond(CondBranches<Ext>, Option<CondElseBranch<Ext>>),
}

impl<Ext> Node<Ext> {
    fn eval<C, Ctx, Eff>(&self, ctx: &C, lex: &mut Lex<Ext>) -> Outcome<Ext, Eff>
    where
        C: Context<Ctx, Ext, Eff>,
        Ext: External,
        Eff: Effect,
    {
        match self {
            Self::Failure => Outcome::Failure,
            Self::Success => Outcome::Success,
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
            Self::Random(seed, ctx_seeds, branches, check_any) => {
                let mut branches: SmallVec::<[_; 16]> = branches.iter().cloned().collect();
                let mut seed = *seed;
                for ctx_seed in ctx_seeds.iter() {
                    let ctx_seed = ctx.tree().ids.get(*ctx_seed)(ctx.view());
                    seed = seed.wrapping_add(ctx_seed);
                }
                let rng = Rng::with_seed(seed);
                rng.shuffle(&mut branches);
                while let Some(node) = branches.pop() {
                    let result = node.eval(ctx, lex);
                    if result.is_success() {
                        return result;
                    }
                    if result.is_action() {
                        if *check_any {
                            for node in branches {
                                if node.eval(ctx, lex).is_success() {
                                    return Outcome::Success;
                                }
                            }
                        }
                        return result;
                    }
                }
                Outcome::Failure
            },
            Self::Cond(branches, else_branch) => {
                'branches: for (branch_cond, branch_body) in branches.iter() {
                    match branch_cond.eval(ctx, lex) {
                        Outcome::Success => {
                            return branch_body.eval(ctx, lex);
                        },
                        Outcome::Failure => {
                            continue 'branches;
                        },
                        other => {
                            return other;
                        },
                    }
                }
                if let Some(else_branch) = else_branch.as_ref() {
                    else_branch.eval(ctx, lex)
                } else {
                    Outcome::Failure
                }
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
        Ext: External,
        Eff: Effect,
    {
        let ctx = mode.apply(ctx);
        let res = ctx.cache().get(*self, arguments, ctx.is_active(), || {
            trace!("eval: {}{:?}", ctx.tree().ids.ref_name(*self), arguments);
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
                Self::Custom(index) => {
                    let node = ctx.tree().ids.get(*index);
                    node(ctx.view(), arguments, ctx.tree(), ctx.is_active(), index.as_seed())
                },
            }
        });
        trace!("outcome: {}{:?} => {:?}", ctx.tree().ids.ref_name(*self), arguments, res);
        res
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
    Ext: External,
    Eff: Effect,
{
    Dispatch::Sequence.eval_branches(ctx, lex, nodes)
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
        Ext: External,
        Eff: Effect,
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
        Ext: External,
        Eff: Effect,
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
                    'values: for topic_value in iter {
                        lex.truncate(lex_len);
                        if !pattern.try_apply(ctx, &mut lex, &topic_value) {
                            continue 'values;
                        }
                        return eval_sequence(ctx, &mut lex, branches);
                    }
                    Outcome::Failure
                })
            },
            Self::Last => {
                let query_fn = ctx.tree().ids.get(index);
                query_fn(ctx.view(), arguments, &mut |iter| {
                    let mut last = Outcome::Failure;
                    'values: for topic_value in iter {
                        lex.truncate(lex_len);
                        if !pattern.try_apply(ctx, &mut lex, &topic_value) {
                            continue 'values;
                        }
                        last = eval_sequence(ctx, &mut lex, branches);
                    }
                    last
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
    Ignore,
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
            Self::Ignore => true,
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