mod build_database;
mod console;
mod context;

use self::{build_database::BuildDatabase, context::Context};
use crate::{
    compile::compile_dynamic,
    error::InfrastructureError,
    ir::{Build, Configuration, Rule},
    parse::parse_dynamic,
    utilities::read_file,
};
use async_recursion::async_recursion;
use futures::future::{join_all, try_join_all, FutureExt, Shared};
use std::{
    collections::hash_map::DefaultHasher,
    future::{ready, Future},
    hash::{Hash, Hasher},
    path::Path,
    pin::Pin,
    sync::Arc,
    time::SystemTime,
};
use tokio::{fs::metadata, io::AsyncWriteExt, process::Command, spawn, sync::Semaphore};

type RawBuildFuture = Pin<Box<dyn Future<Output = Result<(), InfrastructureError>> + Send>>;
type BuildFuture = Shared<RawBuildFuture>;

pub async fn run(
    configuration: Configuration,
    build_directory: &Path,
    job_limit: Option<usize>,
    debug: bool,
) -> Result<(), InfrastructureError> {
    let context = Arc::new(Context::new(
        configuration,
        BuildDatabase::new(build_directory)?,
        Semaphore::new(job_limit.unwrap_or_else(num_cpus::get)),
        debug,
    ));

    for output in context.configuration().default_outputs() {
        create_build_future(
            &context,
            context
                .configuration()
                .outputs()
                .get(output)
                .ok_or_else(|| InfrastructureError::DefaultOutputNotFound(output.into()))?,
        )
        .await?;
    }

    // Do not inline this to avoid borrowing a lock of builds.
    let futures = context
        .builds()
        .read()
        .await
        .values()
        .cloned()
        .collect::<Vec<_>>();

    // Start running build futures actually.
    join_builds(futures).await?;

    Ok(())
}

#[async_recursion]
async fn create_build_future(
    context: &Arc<Context>,
    build: &Arc<Build>,
) -> Result<(), InfrastructureError> {
    // Exclusive lock for atomic addition of a build job.
    let mut builds = context.builds().write().await;

    if builds.contains_key(build.id()) {
        return Ok(());
    }

    let future: RawBuildFuture = Box::pin(spawn_build_future(context.clone(), build.clone()));

    builds.insert(build.id().into(), future.shared());

    Ok(())
}

async fn spawn_build_future(
    context: Arc<Context>,
    build: Arc<Build>,
) -> Result<(), InfrastructureError> {
    spawn(async move {
        let mut futures = vec![];

        for input in build.inputs().iter().chain(build.order_only_inputs()) {
            futures.push(
                if let Some(build) = context.configuration().outputs().get(input) {
                    create_build_future(&context, build).await?;

                    context.builds().read().await[build.id()].clone()
                } else {
                    // TODO Consider registering this future as a build job of the input.
                    let input = input.to_string();
                    let raw: RawBuildFuture =
                        Box::pin(async move { check_file_existence(&input).await });
                    raw.shared()
                },
            );
        }

        join_builds(futures).await?;

        // TODO Consider caching dynamic modules.
        let dynamic_configuration = if let Some(dynamic_module) = build.dynamic_module() {
            Some(compile_dynamic(&parse_dynamic(
                &read_file(&dynamic_module).await?,
            )?)?)
        } else {
            None
        };
        let dynamic_inputs = if let Some(configuration) = &dynamic_configuration {
            build
                .outputs()
                .iter()
                .find_map(|output| configuration.outputs().get(output.as_str()))
                .map(|build| build.inputs())
                .ok_or_else(|| InfrastructureError::DynamicDependencyNotFound(build.clone()))?
        } else {
            &[]
        };

        let mut futures = vec![];

        for input in dynamic_inputs {
            let build = &context.configuration().outputs()[input];

            create_build_future(&context, build).await?;

            futures.push(context.builds().read().await[build.id()].clone());
        }

        join_builds(futures).await?;

        let hash = hash_build(&build, dynamic_inputs).await?;

        if hash == context.database().get(build.id())?
            && try_join_all(
                build
                    .outputs()
                    .iter()
                    .chain(build.implicit_outputs())
                    .map(check_file_existence),
            )
            .await
            .is_ok()
        {
            return Ok(());
        } else if let Some(rule) = build.rule() {
            run_rule(&context, rule).await?;
        }

        context.database().set(build.id(), hash)?;

        Ok(())
    })
    .await?
}

async fn hash_build(build: &Build, dynamic_inputs: &[String]) -> Result<u64, InfrastructureError> {
    let mut hasher = DefaultHasher::new();

    build.rule().map(Rule::command).hash(&mut hasher);
    join_all(
        build
            .inputs()
            .iter()
            .chain(dynamic_inputs)
            .map(get_timestamp),
    )
    .await
    .into_iter()
    .collect::<Result<Vec<SystemTime>, _>>()?
    .hash(&mut hasher);

    Ok(hasher.finish())
}

async fn get_timestamp(path: impl AsRef<Path>) -> Result<SystemTime, InfrastructureError> {
    let path = path.as_ref();

    Ok(metadata(path)
        .await
        .map_err(|error| InfrastructureError::with_path(error, path))?
        .modified()
        .map_err(|error| InfrastructureError::with_path(error, path))?)
}

async fn join_builds(
    builds: impl IntoIterator<Item = BuildFuture>,
) -> Result<(), InfrastructureError> {
    let future: RawBuildFuture = Box::pin(ready(Ok(())));

    try_join_all(builds.into_iter().chain([future.shared()])).await?;

    Ok(())
}

async fn check_file_existence(path: impl AsRef<Path>) -> Result<(), InfrastructureError> {
    let path = path.as_ref();

    metadata(path)
        .await
        .map_err(|error| InfrastructureError::with_path(error, path))?;

    Ok(())
}

async fn run_rule(context: &Context, rule: &Rule) -> Result<(), InfrastructureError> {
    let permit = context.job_semaphore().acquire().await?;
    let output = Command::new("sh")
        .arg("-e")
        .arg("-c")
        .arg(rule.command())
        .output()
        .await?;
    drop(permit);

    let mut console = context.console().lock().await;

    if let Some(description) = rule.description() {
        console.stderr().write_all(description.as_bytes()).await?;
        console.stderr().write_all("\n".as_bytes()).await?;
    }

    console.stdout().write_all(&output.stdout).await?;
    console.stderr().write_all(&output.stderr).await?;

    if !output.status.success() {
        return Err(InfrastructureError::CommandExit(
            rule.command().into(),
            output.status.code(),
        ));
    }

    Ok(())
}
