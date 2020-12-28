use rayon::prelude::*;

use crate::types::{Apps, Atom, Interner, Modules};

pub struct Analyzer {
    modules: Modules,
    apps: Apps,
}

pub enum AnalysisResult {
    MissingModule(Atom),
    MissingFunction(Atom, Atom, u32),
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
        }
    }
}

impl Analyzer {
    pub fn new(modules: Modules, apps: Apps) -> Analyzer {
        Analyzer { modules, apps }
    }

    pub fn run(&self, apps: &[Atom]) -> Vec<(Atom, AnalysisResult)> {
        apps.par_iter()
            .flat_map(|app| self.apps[app].modules.par_iter())
            .flat_map(|&module| {
                let (imports, _) = self.modules.get(&module).unwrap();
                imports.par_iter().flat_map(move |(&from, functions)| {
                    match self.modules.get(&from) {
                        Some((_, exports)) => functions
                            .iter()
                            .filter(|fa| !exports.contains(fa))
                            .map(|(f, a)| (module, AnalysisResult::MissingFunction(from, *f, *a)))
                            .collect(),
                        None => vec![(module, AnalysisResult::MissingModule(from))],
                    }
                })
            })
            .collect()
    }
}
