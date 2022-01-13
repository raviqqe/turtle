mod arguments;
mod ast;
mod compile;
mod error;
mod ir;
mod parse;
mod run;
mod utilities;
mod validation;

use arguments::Arguments;
use ast::{Module, Statement};
use clap::Parser;
use compile::{compile, ModuleDependencyMap};
use error::InfrastructureError;
use futures::future::try_join_all;
use parse::parse;
use run::run;
use std::{
    collections::HashMap,
    env::set_current_dir,
    error::Error,
    path::{Path, PathBuf},
    process::exit,
};

use utilities::{canonicalize_path, read_file};
use validation::{validate_configuration, validate_modules};

const DEFAULT_BUILD_FILE: &str = "build.ninja";
const DEFAULT_LOG_PREFIX: &str = "turtle: ";

#[tokio::main]
async fn main() {
    if let Err(error) = execute().await {
        eprintln!("{}{}", DEFAULT_LOG_PREFIX, error);

        exit(1)
    }
}

async fn execute() -> Result<(), Box<dyn Error>> {
    let arguments = Arguments::parse();

    if let Some(directory) = &arguments.directory {
        set_current_dir(directory)?;
    }

    let root_module_path =
        canonicalize_path(&arguments.file.as_deref().unwrap_or(DEFAULT_BUILD_FILE)).await?;
    let (modules, dependencies) = read_modules(&root_module_path).await?;

    validate_modules(&dependencies)?;

    let configuration = compile(&modules, &dependencies, &root_module_path)?;

    validate_configuration(&configuration)?;

    let build_directory = configuration
        .build_directory()
        .map(PathBuf::from)
        .unwrap_or_else(|| root_module_path.parent().unwrap().into());

    run(configuration, &build_directory, arguments.job_limit).await?;

    Ok(())
}

async fn read_modules(
    path: &Path,
) -> Result<(HashMap<PathBuf, Module>, ModuleDependencyMap), InfrastructureError> {
    let mut paths = vec![canonicalize_path(path).await?];
    let mut modules = HashMap::new();
    let mut dependencies = HashMap::new();

    while let Some(path) = paths.pop() {
        let module = parse(&read_file(&path).await?)?;

        let submodule_paths = try_join_all(
            module
                .statements()
                .iter()
                .filter_map(|statement| match statement {
                    Statement::Include(include) => Some(include.path()),
                    Statement::Submodule(submodule) => Some(submodule.path()),
                    _ => None,
                })
                .map(|submodule_path| resolve_submodule_path(&path, submodule_path))
                .collect::<Vec<_>>(),
        )
        .await?
        .into_iter()
        .collect::<HashMap<_, _>>();

        paths.extend(submodule_paths.values().cloned());

        modules.insert(path.clone(), module);
        dependencies.insert(path, submodule_paths);
    }

    Ok((modules, dependencies))
}

async fn resolve_submodule_path(
    module_path: &Path,
    submodule_path: &str,
) -> Result<(String, PathBuf), InfrastructureError> {
    Ok((
        submodule_path.into(),
        canonicalize_path(module_path.parent().unwrap().join(submodule_path)).await?,
    ))
}
