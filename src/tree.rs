
use id_map::*;
pub use id_space::*;


pub mod outcome;
pub mod id_map;
pub mod id_space;
pub mod script;
pub mod builder;

mod context;

pub struct BehaviorTree<Ctx, Ext, Eff> {
    ids: IdSpace<Ctx, Ext, Eff>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArityError {
    pub expected: usize,
    pub given: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KindError {
    pub expected: Kinds,
    pub given: Kind,
}