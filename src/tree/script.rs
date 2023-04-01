
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

#[derive(Debug, Clone, thiserror::Error)]
pub enum ScriptError {
    #[error(transparent)]
    Find(Arc<walkdir::Error>),
    #[error("Could not read `{}`: {_1}", _0.display())]
    Read(Arc<Path>, Arc<io::Error>),
    #[error(transparent)]
    Compile(CompileError),
    #[error(transparent)]
    Conflict(ConflictError),
}

impl ScriptError {
    pub fn display_full(&self) -> ScriptErrorDisplayFull<'_> {
        ScriptErrorDisplayFull { error: self }
    }
}

pub struct ScriptErrorDisplayFull<'a> {
    error: &'a ScriptError,
}

impl<'a> std::fmt::Display for ScriptErrorDisplayFull<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.error {
            error @ ScriptError::Find(_) | error @ ScriptError::Read(_, _) => error.fmt(f),
            ScriptError::Compile(error) => {
                error.kind.fmt(f)?;
                error.context.fmt(f)?;
                Ok(())
            },
            ScriptError::Conflict(error) => {
                error.kind.fmt(f)?;
                error.context.fmt(f)?;
                if let Some(other_context) = error.cause.context() {
                    other_context.fmt(f)?;
                }
                Ok(())
            },
        }
    }
}
