
pub mod value;
pub mod system;
pub mod loader;


pub trait World: 'static {
    type State;
    type Effect;
    type Value: Clone;
}
