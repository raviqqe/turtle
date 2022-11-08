use super::{build_database::BuildDatabase, options::Options, BuildFuture};
use crate::{
    console::Console,
    ir::{BuildId, Configuration},
    validation::BuildGraph,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, RwLock, Semaphore};

#[derive(Debug)]
pub struct Context<'a> {
    configuration: Arc<Configuration<'a>>,
    // TODO Use a concurrent hash map. We only need atomic insertion but not a great lock.
    build_futures: RwLock<HashMap<BuildId, BuildFuture<'a>>>,
    build_graph: Mutex<BuildGraph>,
    database: BuildDatabase,
    job_semaphore: Semaphore,
    console: Arc<Mutex<Console>>,
    options: Options,
}

impl<'a> Context<'a> {
    pub fn new(
        configuration: Arc<Configuration<'a>>,
        build_graph: BuildGraph,
        database: BuildDatabase,
        job_semaphore: Semaphore,
        console: Arc<Mutex<Console>>,
        options: Options,
    ) -> Self {
        Self {
            build_graph: build_graph.into(),
            configuration,
            build_futures: RwLock::new(HashMap::new()),
            database,
            job_semaphore,
            console,
            options,
        }
    }

    pub fn configuration(&self) -> &Configuration<'a> {
        &self.configuration
    }

    pub fn build_futures(&self) -> &RwLock<HashMap<BuildId, BuildFuture<'a>>> {
        &self.build_futures
    }

    pub fn build_graph(&self) -> &Mutex<BuildGraph> {
        &self.build_graph
    }

    pub fn database(&self) -> &BuildDatabase {
        &self.database
    }

    pub fn job_semaphore(&self) -> &Semaphore {
        &self.job_semaphore
    }

    pub fn console(&self) -> &Mutex<Console> {
        &self.console
    }

    pub fn options(&self) -> &Options {
        &self.options
    }
}
