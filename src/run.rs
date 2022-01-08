mod error;

use crate::ir::{Build, Configuration};
use error::RunError;
use futures::future::{select_all, FutureExt, Shared};
use std::collections::HashMap;
use std::future::{ready, Future};
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::spawn;
use tokio::{io::stderr, process::Command};

type BuildFuture = Shared<Pin<Box<dyn Future<Output = Result<(), RunError>> + Send>>>;

pub async fn run(configuration: &Configuration) -> Result<(), RunError> {
    let mut builds = HashMap::new();

    select_builds(
        configuration
            .outputs()
            .values()
            .map(|build| run_build(configuration, &mut builds, build)),
    )
    .await?;

    Ok(())
}

fn run_build(
    configuration: &Configuration,
    builds: &mut HashMap<String, BuildFuture>,
    build: &Build,
) -> BuildFuture {
    if let Some(future) = builds.get(build.id()) {
        return future.clone();
    }

    let inputs = Arc::new(
        build
            .inputs()
            .iter()
            .map(|input| run_build(configuration, builds, &configuration.outputs()[input]))
            .collect::<Vec<_>>(),
    );

    let rule = build.rule().clone();
    let handle = spawn(async move {
        select_builds(inputs.iter().cloned().collect::<Vec<_>>()).await?;
        run_command(rule.command()).await
    });
    let boxed: Pin<Box<dyn Future<Output = _> + Send>> = Box::pin(async move { handle.await? });
    let future = boxed.shared();

    builds.insert(build.id().into(), future.clone());

    future
}

async fn select_builds(builds: impl IntoIterator<Item = BuildFuture>) -> Result<(), RunError> {
    let future: Pin<Box<dyn Future<Output = _> + Send>> = Box::pin(ready(Ok(())));

    select_all(builds.into_iter().chain([future.shared()]))
        .await
        .0?;

    Ok(())
}

async fn run_command(command: &str) -> Result<(), RunError> {
    let output = Command::new("sh")
        .arg("-e")
        .arg("-c")
        .arg(command)
        .output()
        .await?;

    stderr().write_all(&output.stdout).await?;

    if output.status.success() {
        Ok(())
    } else {
        stderr().write_all(&output.stderr).await?;

        Err(RunError::ChildExit(output.status.code()))
    }
}
