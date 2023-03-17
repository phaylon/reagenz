
pub mod value;
pub mod system;
pub mod loader;

mod core;

pub trait World: 'static {
    type State<'st>: Copy;
    type Effect;
    type Value: Clone + PartialEq + PartialOrd;
}
