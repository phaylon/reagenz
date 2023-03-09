use smallvec::SmallVec;

use crate::World;
use crate::system::{Context, Outcome};
use crate::value::Value;


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
        let arguments = reify_values(&self.arguments, vars);
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
    Ref {
        node: usize,
        arguments: Vec<NodeValue<W>>,
        is_active: bool,
    },
}

impl<W> NodeBranch<W>
where
    W: World,
{
    pub(super) fn eval(&self, ctx: &Context<'_, W>, vars: &mut VarSpace<W>) -> Outcome<W> {
        match self {
            Self::Select { branches } => {
                for branch in branches {
                    let result = branch.eval(ctx, vars);
                    if !result.is_failure() {
                        return result;
                    }
                }
                Outcome::Failure
            },
            Self::Sequence { branches } => {
                for branch in branches {
                    let result = branch.eval(ctx, vars);
                    if !result.is_success() {
                        return result;
                    }
                }
                Outcome::Success
            },
            Self::Ref { node, arguments, is_active } => {
                let ctx = if !is_active { ctx.to_inactive() } else { ctx.clone() };
                let arguments = reify_values(arguments, vars);
                ctx.run_raw(*node, &arguments)
            },
        }
    }
}

fn reify_values<W>(values: &[NodeValue<W>], vars: &[Value<W>]) -> SmallVec<[Value<W>; 8]>
where
    W: World,
{
    values.iter().map(|value| match value {
        NodeValue::Value(value) => value.clone(),
        NodeValue::Variable(index) => vars[*index].clone(),
    }).collect()
}

pub(super) enum NodeValue<W: World> {
    Value(Value<W>),
    Variable(usize),
}
