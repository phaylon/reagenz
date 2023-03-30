use std::borrow::Cow;
use std::cell::RefCell;

use super::{BehaviorTree, ActionIdx};
use super::outcome::{Action, Outcome};


pub trait Context<Ctx, Ext, Eff>: Sized + Clone {
    fn view(&self) -> &Ctx;

    fn tree(&self) -> &BehaviorTree<Ctx, Ext, Eff>;

    fn to_inactive(&self) -> Self;

    fn is_active(&self) -> bool;

    fn action(&self, action: Action<Ext, Eff>) -> Outcome<Ext, Eff>;

    fn to_inactive_if_active(&self) -> Cow<'_, Self> {
        if self.is_active() {
            Cow::Owned(self.to_inactive())
        } else {
            Cow::Borrowed(self)
        }
    }
}

pub struct EvalContext<'a, Ctx, Ext, Eff> {
    view: &'a Ctx,
    tree: &'a BehaviorTree<Ctx, Ext, Eff>,
    is_active: bool,
}

impl<'a, Ctx, Ext, Eff> Clone for EvalContext<'a, Ctx, Ext, Eff> {
    fn clone(&self) -> Self {
        Self {
            view: self.view,
            tree: self.tree,
            is_active: self.is_active,
        }
    }
}

impl<'a, Ctx, Ext, Eff> EvalContext<'a, Ctx, Ext, Eff> {
    pub fn new(view: &'a Ctx, tree: &'a BehaviorTree<Ctx, Ext, Eff>) -> Self {
        Self { view, tree, is_active: true }
    }
}

impl<'a, Ctx, Ext, Eff> Context<Ctx, Ext, Eff> for EvalContext<'a, Ctx, Ext, Eff> {
    fn view(&self) -> &Ctx {
        self.view
    }

    fn tree(&self) -> &BehaviorTree<Ctx, Ext, Eff> {
        self.tree
    }

    fn is_active(&self) -> bool {
        self.is_active
    }

    fn to_inactive(&self) -> Self {
        Self {
            view: self.view,
            tree: self.tree,
            is_active: false,
        }
    }

    fn action(&self, action: Action<Ext, Eff>) -> Outcome<Ext, Eff> {
        if self.is_active {
            Outcome::Action(action)
        } else {
            Outcome::Failure
        }
    }
}

pub struct DiscoveryContext<'ctx, 'coll, Ctx, Ext, Eff, C> {
    view: &'ctx Ctx,
    tree: &'ctx BehaviorTree<Ctx, Ext, Eff>,
    collection: &'ctx RefCell<&'coll mut C>,
    index: ActionIdx,
}

impl<'ctx, 'coll, Ctx, Ext, Eff, C> Clone for DiscoveryContext<'ctx, 'coll, Ctx, Ext, Eff, C> {
    fn clone(&self) -> Self {
        Self {
            view: self.view,
            tree: self.tree,
            collection: self.collection,
            index: self.index,
        }
    }
}

impl<'ctx, 'coll, Ctx, Ext, Eff, C> DiscoveryContext<'ctx, 'coll, Ctx, Ext, Eff, C> {
    pub fn new(
        view: &'ctx Ctx,
        tree: &'ctx BehaviorTree<Ctx, Ext, Eff>,
        collection: &'ctx RefCell<&'coll mut C>,
        index: ActionIdx,
    ) -> Self {
        Self { view, tree, collection, index }
    }
}

impl<'ctx, 'coll, Ctx, Ext, Eff, C> Context<Ctx, Ext, Eff>
for DiscoveryContext<'ctx, 'coll, Ctx, Ext, Eff, C>
where
    C: Extend<Action<Ext, Eff>>,
{
    fn view(&self) -> &Ctx {
        self.view
    }

    fn tree(&self) -> &BehaviorTree<Ctx, Ext, Eff> {
        self.tree
    }

    fn to_inactive(&self) -> Self {
        self.clone()
    }

    fn is_active(&self) -> bool {
        false
    }

    fn action(&self, action: Action<Ext, Eff>) -> Outcome<Ext, Eff> {
        if self.index == action.index() {
            self.collection.borrow_mut().extend([action]);
            Outcome::Success
        } else {
            Outcome::Failure
        }
    }
}