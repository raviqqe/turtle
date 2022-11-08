use super::Rule;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Build<'a> {
    // IDs are persistent across different builds so that they can be used for,
    // for example, caching.
    id: String,
    outputs: Vec<&'a str>,
    implicit_outputs: Vec<&'a str>,
    rule: Option<Rule>,
    inputs: Vec<&'a str>,
    order_only_inputs: Vec<&'a str>,
    dynamic_module: Option<String>,
}

impl<'a> Build<'a> {
    pub fn new(
        outputs: Vec<&'a str>,
        implicit_outputs: Vec<&'a str>,
        rule: Option<Rule>,
        inputs: Vec<&'a str>,
        order_only_inputs: Vec<&'a str>,
        dynamic_module: Option<String>,
    ) -> Self {
        Self {
            id: Self::calculate_id(&outputs, &implicit_outputs),
            outputs,
            implicit_outputs,
            rule,
            inputs,
            order_only_inputs,
            dynamic_module,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn outputs(&self) -> &[&'a str] {
        &self.outputs
    }

    pub fn implicit_outputs(&self) -> &[&'a str] {
        &self.implicit_outputs
    }

    pub fn rule(&self) -> Option<&Rule> {
        self.rule.as_ref()
    }

    pub fn inputs(&self) -> &[&'a str] {
        &self.inputs
    }

    pub fn order_only_inputs(&self) -> &[&'a str] {
        &self.order_only_inputs
    }

    pub fn dynamic_module(&self) -> Option<&str> {
        self.dynamic_module.as_deref()
    }

    fn calculate_id(outputs: &[&'a str], implicit_outputs: &[&'a str]) -> String {
        let mut hasher = DefaultHasher::new();

        outputs.hash(&mut hasher);
        implicit_outputs.hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }
}
