
pub mod value;
pub mod system;
pub mod loader;


pub trait World: 'static {
    type Value: Clone;
    type Effect;
}
