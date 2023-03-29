
use std::borrow::Cow;
use std::io;
use std::path::Path;
use std::sync::Arc;

pub use runtime::*;
pub use compile::*;


mod runtime;
mod compile;

pub type ScriptResult<T = ()> = Result<T, ScriptError>;

#[derive(Clone)]
pub enum ScriptSource<'a> {
    Path { path: Cow<'a, Path> },
    Str { content: &'a str, name: &'a str },
}

#[derive(Debug, Clone)]
pub enum ScriptError {
    Find(Arc<walkdir::Error>),
    Read(Arc<Path>, Arc<io::Error>),
    Compile(CompileError),
    Conflict(ConflictError),
}
