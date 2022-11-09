mod chain_map;
mod context;
mod error;
mod global_state;
mod module_state;

use self::{
    chain_map::ChainMap, context::Context, global_state::GlobalState, module_state::ModuleState,
};
pub use self::{context::ModuleDependencyMap, error::CompileError};
use crate::{
    ast,
    ir::{Build, Configuration, DynamicBuild, DynamicConfiguration, PathSet, Rule},
};
use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

const PHONY_RULE: &str = "phony";
const BUILD_DIRECTORY_VARIABLE: &str = "builddir";
const DYNAMIC_MODULE_VARIABLE: &str = "dyndep";
const SOURCE_VARIABLE_NAME: &str = "srcdep";

static VARIABLE_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$([[:alpha:]_][[:alnum:]_]*)").unwrap());

pub fn compile<'a>(
    modules: &HashMap<PathBuf, ast::Module<'a>>,
    dependencies: &ModuleDependencyMap,
    root_module_path: &Path,
) -> Result<Configuration<'a>, CompileError> {
    let context = Context::new(modules.clone(), dependencies.clone());
    let mut paths = PathSet::new();

    let mut global_state = GlobalState {
        outputs: Default::default(),
        default_outputs: Default::default(),
        source_map: Default::default(),
    };
    let mut module_state = ModuleState {
        rules: ChainMap::new(),
        variables: ChainMap::new(),
    };

    compile_module(
        &context,
        &mut global_state,
        &mut module_state,
        &mut paths,
        root_module_path,
    )?;

    let default_outputs = if global_state.default_outputs.is_empty() {
        global_state.outputs.keys().cloned().collect()
    } else {
        global_state.default_outputs
    };

    Ok(Configuration::new(
        global_state.outputs,
        default_outputs,
        global_state.source_map,
        module_state
            .variables
            .get(BUILD_DIRECTORY_VARIABLE)
            .cloned(),
        paths,
    ))
}

fn compile_module<'a>(
    context: &Context<'a>,
    global_state: &mut GlobalState<'a>,
    module_state: &mut ModuleState<'a, '_>,
    paths: &mut PathSet<'a>,
    path: &Path,
) -> Result<(), CompileError> {
    let module = &context
        .modules()
        .get(path)
        .ok_or_else(|| CompileError::ModuleNotFound(path.into()))?;

    for statement in module.statements() {
        match statement {
            ast::Statement::Build(build) => {
                let mut variables = module_state.variables.fork();

                variables.extend(
                    build
                        .variable_definitions()
                        .iter()
                        .map(|definition| (definition.name().into(), definition.value().into()))
                        .chain([
                            ("in".into(), build.inputs().join(" ")),
                            ("out".into(), build.outputs().join(" ")),
                        ]),
                );

                let ir = Arc::new(Build::new(
                    build.outputs().to_vec(),
                    build.implicit_outputs().to_vec(),
                    if build.rule() == PHONY_RULE {
                        None
                    } else {
                        let rule = &module_state
                            .rules
                            .get(build.rule())
                            .ok_or_else(|| CompileError::RuleNotFound(build.rule().into()))?;

                        Some(Rule::new(
                            interpolate_variables(rule.command(), &variables),
                            rule.description()
                                .map(|description| interpolate_variables(description, &variables)),
                        ))
                    },
                    build
                        .inputs()
                        .iter()
                        .chain(build.implicit_inputs())
                        .copied()
                        .collect(),
                    build.order_only_inputs().to_vec(),
                    variables.get(DYNAMIC_MODULE_VARIABLE).cloned(),
                ));

                let outputs = || build.outputs().iter().chain(build.implicit_outputs());

                global_state
                    .outputs
                    .extend(outputs().map(|&output| (output.to_owned(), ir.clone())));

                if let Some(source) = variables.get(SOURCE_VARIABLE_NAME) {
                    global_state
                        .source_map
                        .extend(outputs().map(|&output| (output.into(), source.clone())));
                }
            }
            ast::Statement::Default(default) => {
                global_state
                    .default_outputs
                    .extend(default.outputs().iter().copied().map(From::from));
            }
            ast::Statement::Include(include) => {
                compile_module(
                    context,
                    global_state,
                    module_state,
                    paths,
                    resolve_dependency(context, path, include.path())?,
                )?;
            }
            ast::Statement::Rule(rule) => {
                module_state.rules.insert(rule.name().into(), rule.clone());
            }
            ast::Statement::Submodule(submodule) => {
                compile_module(
                    context,
                    global_state,
                    &mut module_state.fork(),
                    paths,
                    resolve_dependency(context, path, submodule.path())?,
                )?;
            }
            ast::Statement::VariableDefinition(definition) => {
                module_state
                    .variables
                    .insert(definition.name().into(), definition.value().into());
            }
        }
    }

    Ok(())
}

pub fn compile_dynamic(module: &ast::DynamicModule) -> Result<DynamicConfiguration, CompileError> {
    Ok(DynamicConfiguration::new(
        module
            .builds()
            .iter()
            .map(|build| {
                (
                    build.output().into(),
                    DynamicBuild::new(
                        build
                            .implicit_inputs()
                            .iter()
                            .copied()
                            .map(From::from)
                            .collect(),
                    ),
                )
            })
            .collect(),
    ))
}

fn resolve_dependency<'a>(
    context: &'a Context,
    module_path: &Path,
    submodule_path: &str,
) -> Result<&'a Path, CompileError> {
    Ok(context
        .dependencies()
        .get(module_path)
        .ok_or_else(|| CompileError::ModuleNotFound(module_path.into()))?
        .get(submodule_path)
        .ok_or_else(|| CompileError::ModuleNotFound(submodule_path.into()))?)
}

fn interpolate_variables(template: &str, variables: &ChainMap<String, String>) -> String {
    VARIABLE_PATTERN
        .replace_all(template, |captures: &Captures| {
            variables.get(&captures[1]).cloned().unwrap_or_default()
        })
        .replace("$$", "$")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use fnv::{FnvHashMap, FnvHashSet};
    use once_cell::sync::Lazy;
    use pretty_assertions::assert_eq;

    static ROOT_MODULE_PATH: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("build.ninja"));
    static DEFAULT_DEPENDENCIES: Lazy<ModuleDependencyMap> = Lazy::new(|| {
        [(ROOT_MODULE_PATH.clone(), Default::default())]
            .into_iter()
            .collect()
    });

    fn ast_explicit_build<'a>(
        outputs: Vec<&'a str>,
        rule: &'a str,
        inputs: Vec<&'a str>,
        variable_definitions: Vec<ast::VariableDefinition<'a>>,
    ) -> ast::Build<'a> {
        ast::Build::new(
            outputs,
            vec![],
            rule,
            inputs,
            vec![],
            vec![],
            variable_definitions,
        )
    }

    fn ir_explicit_build<'a>(outputs: Vec<&'a str>, rule: Rule, inputs: Vec<&'a str>) -> Build<'a> {
        Build::new(outputs, vec![], rule.into(), inputs, vec![], None)
    }

    fn create_simple_configuration(
        outputs: FnvHashMap<String, Arc<Build>>,
        default_outputs: FnvHashSet<String>,
    ) -> Configuration {
        Configuration::new(
            outputs,
            default_outputs,
            Default::default(),
            None,
            Default::default(),
        )
    }

    #[test]
    fn compile_empty_module() {
        assert_eq!(
            compile(
                &[(ROOT_MODULE_PATH.clone(), ast::Module::new(vec![]))]
                    .into_iter()
                    .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(Default::default(), Default::default())
        );
    }

    #[test]
    fn interpolate_variable_in_command() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![
                        ast::VariableDefinition::new("x", "42").into(),
                        ast::Rule::new("foo", "$x", None).into(),
                        ast_explicit_build(vec!["bar"], "foo", vec![], vec![]).into(),
                    ])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [(
                    "bar".into(),
                    ir_explicit_build(vec!["bar"], Rule::new("42", None), vec![]).into()
                )]
                .into_iter()
                .collect(),
                ["bar".into()].into_iter().collect()
            )
        );
    }

    #[test]
    fn interpolate_two_variables_in_command() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![
                        ast::VariableDefinition::new("x", "1").into(),
                        ast::VariableDefinition::new("y", "2").into(),
                        ast::Rule::new("foo", "$x $y", None).into(),
                        ast_explicit_build(vec!["bar"], "foo", vec![], vec![]).into(),
                    ])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [(
                    "bar".into(),
                    ir_explicit_build(vec!["bar"], Rule::new("1 2", None), vec![]).into()
                )]
                .into_iter()
                .collect(),
                ["bar".into()].into_iter().collect()
            )
        );
    }

    #[test]
    fn interpolate_variable_with_underscore_in_command() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![
                        ast::VariableDefinition::new("x_y", "42").into(),
                        ast::Rule::new("foo", "$x_y", None).into(),
                        ast_explicit_build(vec!["bar"], "foo", vec![], vec![]).into(),
                    ])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [(
                    "bar".into(),
                    ir_explicit_build(vec!["bar"], Rule::new("42", None), vec![]).into()
                )]
                .into_iter()
                .collect(),
                ["bar".into()].into_iter().collect()
            )
        );
    }

    #[test]
    fn interpolate_dollar_sign_in_command() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![
                        ast::Rule::new("foo", "$$", None).into(),
                        ast_explicit_build(vec!["bar"], "foo", vec![], vec![]).into()
                    ])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [(
                    "bar".into(),
                    ir_explicit_build(vec!["bar"], Rule::new("$", None), vec![]).into()
                )]
                .into_iter()
                .collect(),
                ["bar".into()].into_iter().collect()
            )
        );
    }

    #[test]
    fn interpolate_in_variable_in_command() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![
                        ast::Rule::new("foo", "$in", None).into(),
                        ast_explicit_build(vec!["bar"], "foo", vec!["baz"], vec![]).into(),
                    ])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [(
                    "bar".into(),
                    ir_explicit_build(vec!["bar"], Rule::new("baz", None), vec!["baz"]).into()
                )]
                .into_iter()
                .collect(),
                ["bar".into()].into_iter().collect()
            )
        );
    }

    #[test]
    fn interpolate_in_variable_with_implicit_input() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![
                        ast::Rule::new("foo", "$in", None).into(),
                        ast::Build::new(
                            vec!["bar"],
                            vec![],
                            "foo",
                            vec!["baz"],
                            vec!["blah"],
                            vec![],
                            vec![]
                        )
                        .into(),
                    ])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [(
                    "bar".into(),
                    ir_explicit_build(vec!["bar"], Rule::new("baz", None), vec!["baz", "blah"])
                        .into()
                )]
                .into_iter()
                .collect(),
                ["bar".into()].into_iter().collect()
            )
        );
    }

    #[test]
    fn interpolate_out_variable_in_command() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![
                        ast::Rule::new("foo", "$out", None).into(),
                        ast_explicit_build(vec!["bar"], "foo", vec![], vec![]).into(),
                    ])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [(
                    "bar".into(),
                    ir_explicit_build(vec!["bar"], Rule::new("bar", None), vec![]).into()
                )]
                .into_iter()
                .collect(),
                ["bar".into()].into_iter().collect()
            )
        );
    }

    #[test]
    fn interpolate_out_variable_with_implicit_output() {
        let build = Arc::new(Build::new(
            vec!["bar"],
            vec!["baz"],
            Rule::new("bar", None).into(),
            vec![],
            vec![],
            None,
        ));

        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![
                        ast::Rule::new("foo", "$out", None).into(),
                        ast::Build::new(
                            vec!["bar"],
                            vec!["baz"],
                            "foo",
                            vec![],
                            vec![],
                            vec![],
                            vec![]
                        )
                        .into(),
                    ])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [("baz".into(), build.clone()), ("bar".into(), build)]
                    .into_iter()
                    .collect(),
                ["baz".into(), "bar".into()].into_iter().collect()
            )
        );
    }

    #[test]
    fn compile_order_only_inputs() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![
                        ast::Rule::new("foo", "$in", None).into(),
                        ast::Build::new(
                            vec!["bar"],
                            vec![],
                            "foo",
                            vec![],
                            vec![],
                            vec!["baz"],
                            vec![]
                        )
                        .into(),
                    ])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [(
                    "bar".into(),
                    Build::new(
                        vec!["bar"],
                        vec![],
                        Some(Rule::new("", None)),
                        vec![],
                        vec!["baz"],
                        None
                    )
                    .into()
                )]
                .into_iter()
                .collect(),
                ["bar".into()].into_iter().collect()
            )
        );
    }

    #[test]
    fn generate_build_ids() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![
                        ast::Rule::new("foo", "", None).into(),
                        ast_explicit_build(vec!["bar"], "foo", vec![], vec![]).into(),
                        ast_explicit_build(vec!["baz"], "foo", vec![], vec![]).into()
                    ])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [
                    (
                        "bar".into(),
                        ir_explicit_build(vec!["bar"], Rule::new("", None), vec![]).into()
                    ),
                    (
                        "baz".into(),
                        ir_explicit_build(vec!["baz"], Rule::new("", None), vec![]).into()
                    )
                ]
                .into_iter()
                .collect(),
                ["bar".into(), "baz".into()].into_iter().collect()
            )
        );
    }

    #[test]
    fn interpolate_build_local_variable() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![
                        ast::Rule::new("foo", "$x", None).into(),
                        ast_explicit_build(
                            vec!["bar"],
                            "foo",
                            vec![],
                            vec![ast::VariableDefinition::new("x", "42")]
                        )
                        .into(),
                    ])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [(
                    "bar".into(),
                    ir_explicit_build(vec!["bar"], Rule::new("42", None), vec![]).into()
                )]
                .into_iter()
                .collect(),
                ["bar".into()].into_iter().collect()
            )
        );
    }

    #[test]
    fn compile_source_map() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![
                        ast::Rule::new("foo", "foo", None).into(),
                        ast_explicit_build(
                            vec!["bar"],
                            "foo",
                            vec![],
                            vec![ast::VariableDefinition::new(
                                SOURCE_VARIABLE_NAME,
                                "oh-my-src"
                            )]
                        )
                        .into(),
                    ])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            Configuration::new(
                [(
                    "bar".into(),
                    ir_explicit_build(vec!["bar"], Rule::new("foo", None), vec![]).into()
                )]
                .into_iter()
                .collect(),
                ["bar".into()].into_iter().collect(),
                [("bar".into(), "oh-my-src".into())].into_iter().collect(),
                None,
            )
        );
    }

    #[test]
    fn compile_phony_rule() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![ast_explicit_build(
                        vec!["foo"],
                        "phony",
                        vec!["bar"],
                        vec![]
                    )
                    .into(),])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [(
                    "foo".into(),
                    Build::new(vec!["foo"], vec![], None, vec!["bar"], vec![], None).into()
                )]
                .into_iter()
                .collect(),
                ["foo".into()].into_iter().collect()
            )
        );
    }

    #[test]
    fn compile_build_directory() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![ast::VariableDefinition::new("builddir", "foo").into()])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            Configuration::new(
                Default::default(),
                Default::default(),
                Default::default(),
                Some("foo".into())
            )
        );
    }

    #[test]
    fn compile_dynamic_module_variable() {
        assert_eq!(
            compile(
                &[(
                    ROOT_MODULE_PATH.clone(),
                    ast::Module::new(vec![ast_explicit_build(
                        vec!["foo"],
                        "phony",
                        vec![],
                        vec![ast::VariableDefinition::new("dyndep", "bar")]
                    )
                    .into()])
                )]
                .into_iter()
                .collect(),
                &DEFAULT_DEPENDENCIES,
                &ROOT_MODULE_PATH
            )
            .unwrap(),
            create_simple_configuration(
                [(
                    "foo".into(),
                    Build::new(
                        vec!["foo"],
                        vec![],
                        None,
                        vec![],
                        vec![],
                        Some("bar".into())
                    )
                    .into()
                )]
                .into_iter()
                .collect(),
                ["foo".into()].into_iter().collect()
            )
        );
    }

    mod submodule {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn reference_variable_in_parent_module() {
            const SUBMODULE_PATH: &str = "foo.ninja";

            assert_eq!(
                compile(
                    &[
                        (
                            ROOT_MODULE_PATH.clone(),
                            ast::Module::new(vec![
                                ast::VariableDefinition::new("x", "42").into(),
                                ast::Submodule::new(SUBMODULE_PATH).into(),
                            ])
                        ),
                        (
                            SUBMODULE_PATH.into(),
                            ast::Module::new(vec![
                                ast::Rule::new("foo", "$x", None).into(),
                                ast_explicit_build(vec!["bar"], "foo", vec![], vec![]).into()
                            ])
                        )
                    ]
                    .into_iter()
                    .collect(),
                    &[(
                        ROOT_MODULE_PATH.clone(),
                        [(SUBMODULE_PATH.into(), PathBuf::from(SUBMODULE_PATH))]
                            .into_iter()
                            .collect()
                    )]
                    .into_iter()
                    .collect(),
                    &ROOT_MODULE_PATH
                )
                .unwrap(),
                create_simple_configuration(
                    [(
                        "bar".into(),
                        ir_explicit_build(vec!["bar"], Rule::new("42", None), vec![]).into()
                    )]
                    .into_iter()
                    .collect(),
                    ["bar".into()].into_iter().collect()
                )
            );
        }

        #[test]
        fn reference_rule_in_parent_module() {
            const SUBMODULE_PATH: &str = "foo.ninja";

            assert_eq!(
                compile(
                    &[
                        (
                            ROOT_MODULE_PATH.clone(),
                            ast::Module::new(vec![
                                ast::VariableDefinition::new("x", "42").into(),
                                ast::Rule::new("foo", "$x", None).into(),
                                ast::Submodule::new(SUBMODULE_PATH).into(),
                            ])
                        ),
                        (
                            SUBMODULE_PATH.into(),
                            ast::Module::new(vec![ast_explicit_build(
                                vec!["bar"],
                                "foo",
                                vec![],
                                vec![]
                            )
                            .into()])
                        )
                    ]
                    .into_iter()
                    .collect(),
                    &[(
                        ROOT_MODULE_PATH.clone(),
                        [(SUBMODULE_PATH.into(), PathBuf::from(SUBMODULE_PATH))]
                            .into_iter()
                            .collect()
                    )]
                    .into_iter()
                    .collect(),
                    &ROOT_MODULE_PATH
                )
                .unwrap(),
                create_simple_configuration(
                    [(
                        "bar".into(),
                        ir_explicit_build(vec!["bar"], Rule::new("42", None), vec![]).into()
                    )]
                    .into_iter()
                    .collect(),
                    ["bar".into()].into_iter().collect()
                )
            );
        }

        #[test]
        fn do_not_overwrite_variable_in_parent_module() {
            const SUBMODULE_PATH: &str = "foo.ninja";

            assert_eq!(
                compile(
                    &[
                        (
                            ROOT_MODULE_PATH.clone(),
                            ast::Module::new(vec![
                                ast::VariableDefinition::new("x", "42").into(),
                                ast::Rule::new("foo", "$x", None).into(),
                                ast::Submodule::new(SUBMODULE_PATH).into(),
                                ast_explicit_build(vec!["bar"], "foo", vec![], vec![]).into(),
                            ])
                        ),
                        (
                            SUBMODULE_PATH.into(),
                            ast::Module::new(vec![ast::VariableDefinition::new("x", "13").into(),])
                        )
                    ]
                    .into_iter()
                    .collect(),
                    &[(
                        ROOT_MODULE_PATH.clone(),
                        [(SUBMODULE_PATH.into(), PathBuf::from(SUBMODULE_PATH))]
                            .into_iter()
                            .collect()
                    )]
                    .into_iter()
                    .collect(),
                    &ROOT_MODULE_PATH
                )
                .unwrap(),
                create_simple_configuration(
                    [(
                        "bar".into(),
                        ir_explicit_build(vec!["bar"], Rule::new("42", None), vec![]).into()
                    )]
                    .into_iter()
                    .collect(),
                    ["bar".into()].into_iter().collect()
                )
            );
        }
    }
}
