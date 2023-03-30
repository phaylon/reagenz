use std::borrow::Cow;
use std::cell::RefCell;

use derivative::Derivative;

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

#[derive(Derivative)]
#[derivative(Clone(bound=""))]
pub struct EvalContext<'a, Ctx, Ext, Eff> {
    view: &'a Ctx,
    tree: &'a BehaviorTree<Ctx, Ext, Eff>,
    is_active: bool,
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

#[derive(Derivative)]
#[derivative(Clone(bound=""))]
pub struct DiscoveryContext<'a, Ctx, Ext, Eff, C> {
    view: &'a Ctx,
    tree: &'a BehaviorTree<Ctx, Ext, Eff>,
    collection: &'a RefCell<C>,
    index: ActionIdx,
}

impl<'a, Ctx, Ext, Eff, C> DiscoveryContext<'a, Ctx, Ext, Eff, C> {
    pub fn new(
        view: &'a Ctx,
        tree: &'a BehaviorTree<Ctx, Ext, Eff>,
        collection: &'a RefCell<C>,
        index: ActionIdx,
    ) -> Self {
        Self { view, tree, collection, index }
    }
}

impl<'a, Ctx, Ext, Eff, C> Context<Ctx, Ext, Eff> for DiscoveryContext<'a, Ctx, Ext, Eff, C>
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