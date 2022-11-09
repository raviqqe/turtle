use super::error::ValidationError;
use crate::ir::{Build, DynamicConfiguration, PathId};
use fnv::FnvHashMap;
use petgraph::{
    algo::{kosaraju_scc, toposort},
    graph::{DefaultIx, NodeIndex},
    Graph,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug)]
pub struct BuildGraph {
    graph: Graph<PathId, ()>,
    nodes: HashMap<PathId, NodeIndex<DefaultIx>>,
    primary_outputs: HashMap<PathId, PathId>,
}

impl BuildGraph {
    pub fn new(outputs: &FnvHashMap<PathId, Arc<Build>>) -> Self {
        let mut this = Self {
            graph: Graph::<PathId, ()>::new(),
            nodes: HashMap::<PathId, NodeIndex<DefaultIx>>::new(),
            primary_outputs: HashMap::new(),
        };

        for (output, build) in outputs {
            for input in build.inputs().iter().chain(build.order_only_inputs()) {
                this.add_edge(output, input);
            }

            // Is this output primary?
            if output == build.outputs()[0] {
                this.primary_outputs.insert(output.into(), output.into());

                for &secondary in build.outputs().iter().skip(1) {
                    this.add_edge(secondary, output);
                    this.primary_outputs.insert(secondary.into(), output.into());
                }
            }
        }

        this
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Err(cycle) = toposort(&self.graph, None) {
            let mut components = kosaraju_scc(&self.graph);

            components.sort_by_key(|component| component.len());

            return Err(ValidationError::CircularBuildDependency(
                components
                    .into_iter()
                    .rev()
                    .find(|component| component.contains(&cycle.node_id()))
                    .unwrap()
                    .into_iter()
                    .map(|id| self.graph[id].clone())
                    .collect(),
            ));
        }

        Ok(())
    }

    pub fn validate_dynamic(
        &mut self,
        configuration: &DynamicConfiguration,
    ) -> Result<(), ValidationError> {
        for (output, build) in configuration.outputs() {
            for input in build.inputs() {
                self.add_edge(self.primary_outputs[&output].clone(), input);
            }
        }

        self.validate()
    }

    fn add_edge(&mut self, output: PathId, input: PathId) {
        self.add_node(output);
        self.add_node(input);

        self.graph
            .add_edge(self.nodes[&output], self.nodes[&input], ());
    }

    fn add_node(&mut self, output: PathId) {
        if !self.nodes.contains_key(&output) {
            self.nodes
                .insert(output.into(), self.graph.add_node(output.into()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{DynamicBuild, Rule};

    fn validate_builds(
        dependencies: &FnvHashMap<String, Arc<Build>>,
    ) -> Result<(), ValidationError> {
        BuildGraph::new(dependencies).validate()
    }

    fn explicit_build<'a>(outputs: Vec<&'a str>, inputs: Vec<&'a str>) -> Build<'a> {
        Build::new(
            outputs,
            vec![],
            Rule::new("", None).into(),
            inputs,
            vec![],
            None,
        )
    }

    #[test]
    fn validate_empty() {
        assert_eq!(validate_builds(&Default::default()), Ok(()));
    }

    #[test]
    fn validate_build_without_input() {
        assert_eq!(
            validate_builds(
                &[("foo".into(), explicit_build(vec!["foo"], vec![]).into())]
                    .into_iter()
                    .collect()
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_build_with_explicit_input() {
        assert_eq!(
            validate_builds(
                &[(
                    "foo".into(),
                    explicit_build(vec!["foo"], vec!["bar"]).into()
                )]
                .into_iter()
                .collect()
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_build_with_order_only_input() {
        assert_eq!(
            validate_builds(
                &[(
                    "foo".into(),
                    Build::new(
                        vec!["foo"],
                        vec![],
                        Rule::new("", None).into(),
                        vec![],
                        vec!["bar"],
                        None
                    )
                    .into()
                )]
                .into_iter()
                .collect()
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_circular_build_with_explicit_input() {
        assert_eq!(
            validate_builds(
                &[(
                    "foo".into(),
                    explicit_build(vec!["foo"], vec!["foo"]).into()
                )]
                .into_iter()
                .collect()
            ),
            Err(ValidationError::CircularBuildDependency(vec!["foo".into()]))
        );
    }

    #[test]
    fn validate_circular_build_with_order_only_input() {
        assert_eq!(
            validate_builds(
                &[(
                    "foo".into(),
                    Build::new(
                        vec!["foo"],
                        vec![],
                        Rule::new("", None).into(),
                        vec![],
                        vec!["foo"],
                        None
                    )
                    .into()
                )]
                .into_iter()
                .collect()
            ),
            Err(ValidationError::CircularBuildDependency(vec!["foo".into()]))
        );
    }

    #[test]
    fn validate_two_builds() {
        assert_eq!(
            validate_builds(
                &[
                    (
                        "foo".into(),
                        explicit_build(vec!["foo"], vec!["bar"]).into()
                    ),
                    ("bar".into(), explicit_build(vec!["bar"], vec![]).into())
                ]
                .into_iter()
                .collect()
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_two_circular_builds() {
        assert_eq!(
            validate_builds(
                &[
                    (
                        "foo".into(),
                        explicit_build(vec!["foo"], vec!["bar"]).into()
                    ),
                    (
                        "bar".into(),
                        explicit_build(vec!["bar"], vec!["foo"]).into()
                    )
                ]
                .into_iter()
                .collect()
            ),
            Err(ValidationError::CircularBuildDependency(vec![
                "foo".into(),
                "bar".into(),
            ]))
        );
    }

    #[test]
    fn validate_with_dynamic_configuration() {
        let mut graph = BuildGraph::new(
            &[
                (
                    "foo".into(),
                    explicit_build(vec!["foo"], vec!["bar"]).into(),
                ),
                ("bar".into(), explicit_build(vec!["bar"], vec![]).into()),
            ]
            .into_iter()
            .collect(),
        );

        graph.validate().unwrap();

        assert_eq!(
            graph.validate_dynamic(&DynamicConfiguration::new(
                [("bar".into(), DynamicBuild::new(vec!["foo".into()]))]
                    .into_iter()
                    .collect(),
            )),
            Err(ValidationError::CircularBuildDependency(vec![
                "foo".into(),
                "bar".into(),
            ]))
        );
    }

    #[test]
    fn validate_circular_build_with_dependency_from_secondary_to_primary() {
        let build = Arc::new(explicit_build(vec!["foo", "bar"], vec![]));

        let mut graph = BuildGraph::new(
            &[("foo".into(), build.clone()), ("bar".into(), build)]
                .into_iter()
                .collect(),
        );

        graph.validate().unwrap();

        assert_eq!(
            graph.validate_dynamic(&DynamicConfiguration::new(
                [("bar".into(), DynamicBuild::new(vec!["foo".into()]))]
                    .into_iter()
                    .collect(),
            )),
            Err(ValidationError::CircularBuildDependency(vec!["foo".into()]))
        );
    }

    #[test]
    fn validate_circular_build_with_dependency_from_primary_to_secondary() {
        let build = Arc::new(explicit_build(vec!["foo", "bar"], vec![]));

        let mut graph = BuildGraph::new(
            &[("foo".into(), build.clone()), ("bar".into(), build)]
                .into_iter()
                .collect(),
        );

        graph.validate().unwrap();

        assert_eq!(
            graph.validate_dynamic(&DynamicConfiguration::new(
                [("foo".into(), DynamicBuild::new(vec!["bar".into()]))]
                    .into_iter()
                    .collect(),
            )),
            Err(ValidationError::CircularBuildDependency(vec![
                "bar".into(),
                "foo".into()
            ]))
        );
    }
}
