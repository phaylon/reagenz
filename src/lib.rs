
mod value;
mod system;

pub use value::*;
pub use system::*;


pub trait World {
    type Value: Clone;
    type Effect;
}
