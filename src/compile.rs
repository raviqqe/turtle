use crate::{
    ast::Module,
    ir::{Build, Configuration, Rule},
};
use std::{collections::HashMap, sync::Arc};

pub fn compile(module: &Module) -> Result<Configuration, String> {
    let variables = [("$", "$")]
        .into_iter()
        .chain(
            module
                .variable_definitions()
                .iter()
                .map(|definition| (definition.name(), definition.value())),
        )
        .collect::<HashMap<_, _>>();

    let rules = module
        .rules()
        .iter()
        .map(|rule| {
            (
                rule.name(),
                Arc::new(Rule::new(
                    interpolate_variables(rule.command(), &variables),
                    rule.description(),
                )),
            )
        })
        .collect::<HashMap<_, _>>();

    Ok(Configuration::new(
        module
            .builds()
            .iter()
            .flat_map(|build| {
                let ir = Arc::new(Build::new(
                    rules[build.rule()].clone(),
                    build.inputs().to_vec(),
                ));

                build
                    .outputs()
                    .iter()
                    .map(|output| (output.clone(), ir.clone()))
                    .collect::<Vec<_>>()
            })
            .collect(),
    ))
}

// TODO Use rsplit to prevent overlapped interoplation.
fn interpolate_variables(template: &str, variables: &HashMap<&str, &str>) -> String {
    variables
        .iter()
        .fold(template.into(), |template, (name, value)| {
            template.replace(&("$".to_string() + name), value)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ast, ir};

    #[test]
    fn compile_empty_module() {
        assert_eq!(
            compile(&ast::Module::new(vec![], vec![], vec![], vec![])).unwrap(),
            ir::Configuration::new(Default::default())
        );
    }

    #[test]
    fn interpolate_variable_in_command() {
        assert_eq!(
            compile(&ast::Module::new(
                vec![ast::VariableDefinition::new("x", "42")],
                vec![ast::Rule::new("foo", "$x", "")],
                vec![ast::Build::new(vec!["bar".into()], "foo", vec![])],
                vec![]
            ))
            .unwrap(),
            ir::Configuration::new(
                [(
                    "bar".into(),
                    Build::new(Rule::new("42", "").into(), vec![]).into()
                )]
                .into_iter()
                .collect()
            )
        );
    }

    #[test]
    fn interpolate_dollar_sign_in_command() {
        assert_eq!(
            compile(&ast::Module::new(
                vec![ast::VariableDefinition::new("x", "42")],
                vec![ast::Rule::new("foo", "$$", "")],
                vec![ast::Build::new(vec!["bar".into()], "foo", vec![])],
                vec![]
            ))
            .unwrap(),
            ir::Configuration::new(
                [(
                    "bar".into(),
                    Build::new(Rule::new("$", "").into(), vec![]).into()
                )]
                .into_iter()
                .collect()
            )
        );
    }
}
