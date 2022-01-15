use crate::{compile::CompileError, ir::Build, parse::ParseError, validation::ValidationError};
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    path::Path,
    sync::Arc,
};
use tokio::{io, sync::AcquireError, task::JoinError};

#[derive(Clone, Debug)]
pub enum InfrastructureError {
    CommandExit(String, Option<i32>),
    Compile(CompileError),
    DefaultOutputNotFound(String),
    DynamicDependencyNotFound(Arc<Build>),
    InputNotFound(String),
    Other(String),
    Parse(ParseError),
    Sled(sled::Error),
    Validation(ValidationError),
}

impl InfrastructureError {
    pub fn with_path(error: io::Error, path: impl AsRef<Path>) -> Self {
        Self::Other(format!("{}: {}", error, path.as_ref().display()))
    }
}

impl Error for InfrastructureError {}

impl Display for InfrastructureError {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            Self::CommandExit(command, code) => {
                write!(
                    formatter,
                    "command exited {}: {}",
                    if let Some(code) = code {
                        format!("with status code {}", code)
                    } else {
                        "without status code".into()
                    },
                    command
                )
            }
            Self::Compile(error) => write!(formatter, "{}", error),
            Self::DefaultOutputNotFound(output) => {
                write!(formatter, "default output \"{}\" not found", output)
            }
            Self::DynamicDependencyNotFound(build) => {
                write!(
                    formatter,
                    "outputs {} not found in dynamic dependency file {}",
                    build.outputs().join(", "),
                    build.dynamic_module().unwrap()
                )
            }
            Self::InputNotFound(output) => {
                write!(formatter, "input \"{}\" not found", output)
            }
            Self::Other(message) => write!(formatter, "{}", message),
            Self::Parse(error) => write!(formatter, "{}", error),
            Self::Sled(error) => write!(formatter, "{}", error),
            Self::Validation(error) => write!(formatter, "{}", error),
        }
    }
}

impl From<AcquireError> for InfrastructureError {
    fn from(error: AcquireError) -> Self {
        Self::Other(format!("{}", &error))
    }
}

impl From<CompileError> for InfrastructureError {
    fn from(error: CompileError) -> Self {
        Self::Compile(error)
    }
}

impl From<io::Error> for InfrastructureError {
    fn from(error: io::Error) -> Self {
        Self::Other(format!("{}", &error))
    }
}

impl From<JoinError> for InfrastructureError {
    fn from(error: JoinError) -> Self {
        Self::Other(format!("{}", &error))
    }
}

impl From<ParseError> for InfrastructureError {
    fn from(error: ParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<sled::Error> for InfrastructureError {
    fn from(error: sled::Error) -> Self {
        Self::Sled(error)
    }
}

impl From<ValidationError> for InfrastructureError {
    fn from(error: ValidationError) -> Self {
        Self::Validation(error)
    }
}
