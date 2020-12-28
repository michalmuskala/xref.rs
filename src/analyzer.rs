use fxhash::FxHashMap;
use petgraph::algo;
use rayon::prelude::*;

use crate::types::{AppDeps, AppModules, Atom, Interner, Modules};

pub struct Analyzer {
    modules: Modules,
    modules_rev: FxHashMap<Atom, Atom>,
    app_modules: AppModules,
    app_deps: AppDeps,
}

pub enum AnalysisResult {
    MissingModule(Atom),
    MissingFunction(Atom, Atom, u32),
    MissingDependency {
        module: Atom,
        app_from: Atom,
        app_to: Atom,
    },
}

impl AnalysisResult {
    pub fn fmt(&self, interner: &Interner) -> String {
        match self {
            AnalysisResult::MissingModule(module) => {
                format!("undefined module: {}", module.resolve(interner).unwrap())
            }
            AnalysisResult::MissingFunction(module, fun, arity) => format!(
                "undefined function: {}:{}/{}",
                module.resolve(interner).unwrap(),
                fun.resolve(interner).unwrap(),
                arity
            ),
            AnalysisResult::MissingDependency { module, app_from, app_to } => format!(
                "missing dependency between applications: application {} uses module {} from {} without depending on it",
                app_from.resolve(interner).unwrap(),
                module.resolve(interner).unwrap(),
                app_to.resolve(interner).unwrap()
            ),
        }
    }
}

impl Analyzer {
    pub fn new(modules: Modules, app_modules: AppModules, app_deps: AppDeps) -> Analyzer {
        let modules_rev = app_modules
            .iter()
            .flat_map(|(&app, modules)| modules.iter().map(move |&module| (module, app)))
            .collect();

        Analyzer {
            modules,
            modules_rev,
            app_modules,
            app_deps,
        }
    }

    pub fn run(&self, apps: &[Atom]) -> Vec<(Atom, AnalysisResult)> {
        apps.par_iter()
            .flat_map(|app| self.app_modules[app].par_iter())
            .flat_map(|&module| {
                let (imports, _) = self.modules.get(&module).unwrap();
                imports.par_iter().flat_map(move |(&imported, functions)| {
                    let mut results = vec![];
                    results.append(&mut self.check_missing_module(module, imported, functions));
                    results.append(&mut self.check_missing_dep(module, imported));
                    results
                })
            })
            .collect()
    }

    fn check_missing_module(
        &self,
        module: Atom,
        imported: Atom,
        functions: &[(Atom, u32)],
    ) -> Vec<(Atom, AnalysisResult)> {
        match self.modules.get(&imported) {
            Some((_, exports)) => functions
                .iter()
                .filter(|fa| !exports.contains(fa))
                .map(|(f, a)| (module, AnalysisResult::MissingFunction(imported, *f, *a)))
                .collect(),
            None => vec![(module, AnalysisResult::MissingModule(imported))],
        }
    }

    fn check_missing_dep(&self, module: Atom, imported: Atom) -> Vec<(Atom, AnalysisResult)> {
        let app_from = self.modules_rev[&module];

        if let Some(&app_to) = self.modules_rev.get(&imported) {
            if algo::has_path_connecting(&self.app_deps, app_from, app_to, None) {
                vec![]
            } else {
                vec![(module, AnalysisResult::MissingDependency { module: imported, app_from, app_to })]
            }
        } else {
            vec![]
        }
    }
}
