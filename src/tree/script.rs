
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

impl ScriptSource {
    pub fn from_path<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self::Path { path: path.as_ref().into() }
    }

    pub fn from_named(name: &str, content: Box<str>) -> Self {
        Self::Str { name: name.into(), content }
    }
}
