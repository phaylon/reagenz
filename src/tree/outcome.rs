use std::sync::Arc;

use derivative::Derivative;

use crate::value::{Value, Values};

use super::id_space::ActionIdx;


#[derive(Derivative, Debug, PartialEq)]
#[derivative(Clone(bound=""))]
pub enum Outcome<Ext, Eff> {
    Success,
    Failure,
    Action(Action<Ext, Eff>),
}

impl<Ext, Eff> Outcome<Ext, Eff> {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    pub fn is_non_success(&self) -> bool {
        !self.is_success()
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failure)
    }

    pub fn is_non_failure(&self) -> bool {
        !self.is_failure()
    }

    pub fn is_action(&self) -> bool {
        matches!(self, Self::Action(_))
    }

    pub fn is_non_action(&self) -> bool {
        !self.is_action()
    }

    pub fn effects(&self) -> Option<&[Eff]> {
        if let Self::Action(action) = self {
            Some(&action.effects)
        } else {
            None
        }
    }
}

impl<Ext, Eff> From<bool> for Outcome<Ext, Eff> {
    fn from(value: bool) -> Self {
        if value {
            Self::Success
        } else {
            Self::Failure
        }
    }
}

#[derive(Derivative, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derivative(Clone(bound=""))]
pub struct Action<Ext, Eff> {
    index: ActionIdx,
    arguments: Values<Ext>,
    effects: Arc<[Eff]>,
}

impl<Ext, Eff> Action<Ext, Eff> {
    pub(super) fn new(index: ActionIdx, arguments: Values<Ext>, effects: Arc<[Eff]>) -> Self {
        Self { index, arguments, effects }
    }

    pub(super) fn index(&self) -> ActionIdx {
        self.index
    }

    pub fn arguments(&self) -> &[Value<Ext>] {
        &self.arguments
    }

    pub fn effects(&self) -> &[Eff] {
        &self.effects
    }
}
