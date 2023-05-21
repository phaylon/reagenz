
use std::cell::RefCell;

use id_map::*;
pub use id_space::*;
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::value::IntoValues;
use crate::{Outcome, Action, Value};

use self::context::{EvalContext, DiscoveryContext, Context, ContextCache};


pub mod outcome;
pub mod id_map;
pub mod id_space;
pub mod script;
pub mod builder;

mod context;

#[derive(derivative::Derivative)]
#[derivative(Clone(bound=""))]
pub struct BehaviorTree<Ctx, Ext, Eff> {
    ids: IdSpace<Ctx, Ext, Eff>,
}

impl<Ctx, Ext, Eff> BehaviorTree<Ctx, Ext, Eff>
where
    Ext: External,
    Eff: Effect,
{
    fn eval_node(
        &self,
        ctx: EvalContext<Ctx, Ext, Eff>,
        node: &str,
        arguments: &[Value<Ext>],
    ) -> Result<Outcome<Ext, Eff>, IdError> {
        match self.ids.resolve_ref(node, arguments.len())? {
            RefIdx::Action(index) => Ok(self.ids.get(index).eval(&ctx, &arguments)),
            RefIdx::Node(index) => Ok(self.ids.get(index).eval(&ctx, &arguments)),
            RefIdx::Cond(index) => Ok(self.ids.get(index)(ctx.view(), &arguments).into()),
            RefIdx::Custom(index) => {
                let seed = index.as_seed();
                Ok(self.ids.get(index)(ctx.view(), &arguments, self, ctx.is_active(), seed))
            },
        }
    }

    pub fn evaluate<A>(
        &self,
        view: &Ctx,
        root: &str,
        arguments: A,
    ) -> Result<Outcome<Ext, Eff>, IdError>
    where
        A: IntoValues<Ext>,
    {
        let ctx = EvalContext::new(view, self);
        let arguments: SmallVec<[_; 8]> = arguments.into_values();
        self.eval_node(ctx, root, &arguments)
    }

    pub fn check<A>(
        &self,
        view: &Ctx,
        root: &str,
        arguments: A,
    ) -> Result<Outcome<Ext, Eff>, IdError>
    where
        A: IntoValues<Ext>,
    {
        let ctx = EvalContext::new(view, self).to_inactive();
        let arguments: SmallVec<[_; 8]> = arguments.into_values();
        self.eval_node(ctx, root, &arguments[..])
    }

    pub fn discover_all<C>(&self, view: &Ctx, collection: &mut C)
    where
        C: Extend<Action<Ext, Eff>>,
    {
        let collection = RefCell::new(collection);
        let cache = ContextCache::default();
        for index in self.ids.actions() {
            let ctx = DiscoveryContext::new(view, self, &collection, index, cache.clone());
            self.ids.get(index).eval_discovery_nodes(&ctx);
        }
    }

    pub fn discover<C>(&self, view: &Ctx, action: &str, collection: &mut C) -> Result<(), IdError>
    where
        C: Extend<Action<Ext, Eff>>,
    {
        let collection = RefCell::new(collection);
        let cache = ContextCache::default();
        let index = self.ids.action(action)?;
        let ctx = DiscoveryContext::new(view, self, &collection, index, cache);
        self.ids.get(index).eval_discovery_nodes(&ctx);
        Ok(())
    }

    #[track_caller]
    pub fn action_name(&self, action: &Action<Ext, Eff>) -> &SmolStr {
        self.ids.action_name(action.index())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
#[error("Expected {expected}, given {given}")]
pub struct ArityError {
    pub expected: usize,
    pub given: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
#[error("Expected {}, given {given}", expected.display_connected("or"))]
pub struct KindError {
    pub expected: Kinds,
    pub given: Kind,
}

pub trait Effect: Sized + Clone + Eq + std::hash::Hash + std::fmt::Debug + 'static {}
impl<T: Sized + Clone + Eq + std::hash::Hash + std::fmt::Debug + 'static> Effect for T {}

pub trait External:  Sized + Clone + Eq + std::hash::Hash + std::fmt::Debug + 'static {}
impl<T: Sized + Clone + Eq + std::hash::Hash + std::fmt::Debug + 'static> External for T {}
