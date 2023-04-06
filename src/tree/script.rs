
use std::path::Path;
use std::sync::Arc;

pub use runtime::*;
pub use compile::*;


mod runtime;
mod compile;

#[derive(Clone)]
pub enum ScriptSource {
    Path { path: Arc<Path> },
    Str { content: Box<str>, name: Arc<str> },
}
