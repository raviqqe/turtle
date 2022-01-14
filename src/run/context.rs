use super::{build_database::BuildDatabase, console::Console, BuildFuture};
use crate::{ir::Configuration, validation::BuildGraph};
use std::collections::HashMap;
use tokio::sync::{Mutex, RwLock, Semaphore};

#[derive(Debug)]
pub struct Context {
    configuration: Configuration,
    // TODO Use a concurrent hash map. We only need atomic insertion but not a great lock.
    build_futures: RwLock<HashMap<String, BuildFuture>>,
    build_graph: Mutex<BuildGraph>,
    database: BuildDatabase,
    job_semaphore: Semaphore,
    console: Mutex<Console>,
    debug: bool,
}

impl Context {
    pub fn new(
        configuration: Configuration,
        build_graph: BuildGraph,
        database: BuildDatabase,
        job_semaphore: Semaphore,
        debug: bool,
    ) -> Self {
        Self {
            build_graph: build_graph.into(),
            configuration,
            build_futures: RwLock::new(HashMap::new()),
            database,
            job_semaphore,
            console: Console::new().into(),
            debug,
        }
    }

    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    pub fn build_futures(&self) -> &RwLock<HashMap<String, BuildFuture>> {
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

    pub fn debug(&self) -> bool {
        self.debug
    }
}
