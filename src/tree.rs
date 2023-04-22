
use std::cell::RefCell;

use id_map::*;
pub use id_space::*;
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::value::IntoValues;
use crate::{Outcome, Action};

use self::context::{EvalContext, DiscoveryContext};


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
    Ext: Clone + PartialEq,
{
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
        match self.ids.resolve_ref(root, arguments.len())? {
            RefIdx::Action(index) => Ok(self.ids.get(index).eval(&ctx, &arguments)),
            RefIdx::Node(index) => Ok(self.ids.get(index).eval(&ctx, &arguments)),
            RefIdx::Cond(index) => Ok(self.ids.get(index)(view, &arguments).into()),
            RefIdx::Custom(index) => Ok(self.ids.get(index)(view, &arguments, self)),
        }
    }

    pub fn discover_all<C>(&self, view: &Ctx, collection: &mut C)
    where
        C: Extend<Action<Ext, Eff>>,
    {
        let collection = RefCell::new(collection);
        for index in self.ids.actions() {
            let ctx = DiscoveryContext::new(view, self, &collection, index);
            self.ids.get(index).eval_discovery_nodes(&ctx);
        }
    }

    pub fn discover<C>(&self, view: &Ctx, action: &str, collection: &mut C) -> Result<(), IdError>
    where
        C: Extend<Action<Ext, Eff>>,
    {
        let collection = RefCell::new(collection);
        let index = self.ids.action(action)?;
        let ctx = DiscoveryContext::new(view, self, &collection, index);
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