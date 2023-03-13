
pub mod value;
pub mod system;
pub mod loader;

mod core;

pub trait World: 'static {
    type State;
    type Effect;
    type Value: Clone + PartialEq;
}
