use crate::ast::Module;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[derive(Debug, Default)]
pub struct CompileContext {
    modules: HashMap<PathBuf, Module>,
    dependencies: HashMap<PathBuf, HashSet<PathBuf>>,
    build_index: RefCell<usize>,
}

impl CompileContext {
    pub fn new(
        modules: HashMap<PathBuf, Module>,
        dependencies: HashMap<PathBuf, HashSet<PathBuf>>,
    ) -> Self {
        Self {
            modules,
            dependencies,
            build_index: RefCell::new(0),
        }
    }

    pub fn modules(&self) -> &HashMap<PathBuf, Module> {
        &self.modules
    }

    pub fn dependencies(&self) -> &HashMap<PathBuf, HashSet<PathBuf>> {
        &self.dependencies
    }

    pub fn generate_build_id(&self) -> String {
        let index = *self.build_index.borrow();

        *self.build_index.borrow_mut() += 1;

        index.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_build_ids() {
        let context = CompileContext::new(Default::default(), Default::default());

        assert_eq!(context.generate_build_id(), "0".to_string());
        assert_eq!(context.generate_build_id(), "1".to_string());
        assert_eq!(context.generate_build_id(), "2".to_string());
    }
}
