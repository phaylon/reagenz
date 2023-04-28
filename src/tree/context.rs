use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use crate::Value;

use super::{BehaviorTree, ActionIdx, RefIdx};
use super::outcome::{Action, Outcome};


pub trait Context<Ctx, Ext, Eff>: Sized + Clone {
    fn view(&self) -> &Ctx;

    fn tree(&self) -> &BehaviorTree<Ctx, Ext, Eff>;

    fn cache(&self) -> &ContextCache<Ext, Eff>;

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
    cache: ContextCache<Ext, Eff>,
}

impl<'a, Ctx, Ext, Eff> Clone for EvalContext<'a, Ctx, Ext, Eff> {
    fn clone(&self) -> Self {
        Self {
            view: self.view,
            tree: self.tree,
            is_active: self.is_active,
            cache: self.cache.clone(),
        }
    }
}

impl<'a, Ctx, Ext, Eff> EvalContext<'a, Ctx, Ext, Eff> {
    pub fn new(view: &'a Ctx, tree: &'a BehaviorTree<Ctx, Ext, Eff>) -> Self {
        Self { view, tree, is_active: true, cache: ContextCache::default() }
    }
}

impl<'a, Ctx, Ext, Eff> Context<Ctx, Ext, Eff> for EvalContext<'a, Ctx, Ext, Eff> {
    fn view(&self) -> &Ctx {
        self.view
    }

    fn tree(&self) -> &BehaviorTree<Ctx, Ext, Eff> {
        self.tree
    }

    fn cache(&self) -> &ContextCache<Ext, Eff> {
        &self.cache
    }

    fn is_active(&self) -> bool {
        self.is_active
    }

    fn to_inactive(&self) -> Self {
        Self {
            view: self.view,
            tree: self.tree,
            is_active: false,
            cache: self.cache.clone(),
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
    cache: ContextCache<Ext, Eff>,
}

impl<'ctx, 'coll, Ctx, Ext, Eff, C> Clone for DiscoveryContext<'ctx, 'coll, Ctx, Ext, Eff, C> {
    fn clone(&self) -> Self {
        Self {
            view: self.view,
            tree: self.tree,
            collection: self.collection,
            index: self.index,
            cache: self.cache.clone(),
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
        Self { view, tree, collection, index, cache: ContextCache::default() }
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

    fn cache(&self) -> &ContextCache<Ext, Eff> {
        &self.cache
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

pub struct ContextCache<Ext, Eff> {
    lru: Rc<RefCell<Vec<CacheLine<Ext, Eff>>>>,
}

impl<Ext, Eff> ContextCache<Ext, Eff>
where
    Ext: Clone + PartialEq,
{
    pub fn get<F>(
        &self,
        ref_index: RefIdx,
        arguments: &[Value<Ext>],
        is_active: bool,
        calc_outcome: F,
    ) -> Outcome<Ext, Eff>
    where
        F: FnOnce() -> Outcome<Ext, Eff>,
    {
        let index = self.lru.borrow().iter().position(|cl| {
            cl.index == ref_index
                && cl.is_active == is_active
                && cl.arguments == arguments
        });
        if let Some(index) = index {
            let mut lru = self.lru.borrow_mut();
            let cl = lru.remove(index);
            let outcome = cl.outcome.clone();
            lru.insert(0, cl);
            outcome
        } else {
            let outcome = calc_outcome();
            let mut lru = self.lru.borrow_mut();
            lru.insert(0, CacheLine {
                index: ref_index,
                is_active,
                arguments: arguments.into(),
                outcome: outcome.clone(),
            });
            lru.truncate(1024);
            outcome
        }
    }
}

impl<Ext, Eff> Default for ContextCache<Ext, Eff> {
    fn default() -> Self {
        Self { lru: Rc::new(RefCell::new(Vec::new())) }
    }
}

impl<Ext, Eff> Clone for ContextCache<Ext, Eff> {
    fn clone(&self) -> Self {
        Self { lru: self.lru.clone() }
    }
}

struct CacheLine<Ext, Eff> {
    index: RefIdx,
    is_active: bool,
    arguments: Vec<Value<Ext>>,
    outcome: Outcome<Ext, Eff>,
}
