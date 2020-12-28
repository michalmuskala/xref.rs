use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::{Context, Result};
use beam_file::{
    chunk::{AtomChunk, ExpTChunk, ImpTChunk, StandardChunk},
    StandardBeamFile,
};
use lazy_static::lazy_static;
use rayon::prelude::*;
use regex::Regex;

use crate::types::{App, Apps, Atom, Exports, Imports, Interner, Modules};

pub struct Loader {
    interner: Mutex<Interner>,
    modules: Mutex<Modules>,
    apps: Mutex<Apps>,
}

impl Loader {
    pub fn new() -> Loader {
        Loader {
            interner: Mutex::new(Interner::new()),
            modules: Mutex::new(Modules::default()),
            apps: Mutex::new(Apps::default()),
        }
    }

    pub fn read_libs(&self, paths: &[PathBuf]) -> Result<()> {
        paths
            .par_iter()
            .flat_map(|path| match fs::read_dir(path) {
                Ok(dirs) => dirs
                    .into_iter()
                    .map(|result| {
                        result.with_context(|| format!("reading lib path: {}", path.display()))
                    })
                    .collect(),
                Err(err) => {
                    vec![Err(err).with_context(|| format!("reading lib path: {}", path.display()))]
                }
            })
            .filter(|entry| {
                entry.as_ref().map_or(true, |entry| {
                    entry
                        .file_name()
                        .to_str()
                        .map_or(false, |name| !name.starts_with("."))
                })
            })
            .try_for_each(|entry| {
                let ebin_path = entry?.path().join("ebin");

                if ebin_path.is_dir() {
                    let app = self.read_app(&ebin_path)?;

                    let mut apps = self.apps.lock().unwrap();
                    apps.insert(app.name, app);
                }

                Ok(())
            })
    }

    pub fn finish(self) -> (Interner, Modules, Apps) {
        (
            self.interner.into_inner().unwrap(),
            self.modules.into_inner().unwrap(),
            self.apps.into_inner().unwrap(),
        )
    }

    fn read_app(&self, ebin_path: &Path) -> Result<App> {
        let mut app_modules = vec![];
        let mut app_name = None;
        let mut app_deps = None;

        for entry in fs::read_dir(ebin_path)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension().and_then(OsStr::to_str) {
                match extension {
                    "beam" => {
                        let (module, imports, exports) =
                            self.read_module(&path).with_context(|| {
                                format!("failed to read BEAM file: {}", path.display())
                            })?;

                        let mut modules = self.modules.lock().unwrap();

                        app_modules.push(module);
                        modules.insert(module, (imports, exports));
                    }
                    "app" => {
                        app_name = path
                            .file_stem()
                            .and_then(OsStr::to_str)
                            .map(|app| Atom(self.interner.lock().unwrap().get_or_intern(app)));
                        app_deps = Some(self.read_app_deps(&path).with_context(|| {
                            format!("failed to parse .app file: {}", path.display())
                        })?)
                    }
                    "appup" | "hrl" | "am" => continue,
                    _ => anyhow::bail!("unexpected file: {:?}", path),
                }
            }
        }

        Ok(App {
            name: app_name
                .with_context(|| format!("missing .app file in {}", ebin_path.display()))?,
            deps: app_deps.unwrap(),
            modules: app_modules,
        })
    }

    fn read_app_deps(&self, path: &Path) -> Result<Vec<Atom>> {
        // This is a very naive way of extracting app dependency information
        // based on a regex, to avoid full parsing. It will probably break
        // at custom-built files, but should be fine with rebar3 emitted ones
        lazy_static! {
            static ref APPS: Regex =
                Regex::new(r"\{\s*(?:included_)?applications\s*,\s*\[\s*([0-9a-z_,\s]+)\s*\]\s*\}")
                    .unwrap();
            static ref COMMA: Regex = Regex::new(r"\s*,\s*").unwrap();
        }

        let text = fs::read_to_string(path)?;

        let deps = {
            let mut interner = self.interner.lock().unwrap();
            APPS.captures_iter(&text)
                .flat_map(|caps| COMMA.split(caps.get(1).unwrap().as_str()))
                .map(|app| Atom(interner.get_or_intern(app)))
                .collect()
        };

        Ok(deps)
    }

    fn read_module(&self, path: &Path) -> Result<(Atom, Imports, Exports)> {
        let beam = StandardBeamFile::from_file(path)?;

        let mut atom_chunk = None;
        let mut import_chunk = None;
        let mut export_chunk = None;

        for chunk in beam.chunks {
            match chunk {
                StandardChunk::Atom(atom) => atom_chunk = Some(atom),
                StandardChunk::ExpT(export) => export_chunk = Some(export),
                StandardChunk::ImpT(import) => import_chunk = Some(import),
                _ => continue,
            }
        }

        let atoms = {
            let mut interner = self.interner.lock().unwrap();
            load_atoms(&mut interner, &atom_chunk.unwrap())
        };
        let imports = load_imports(&atoms, &import_chunk.unwrap());
        let exports = load_exports(&atoms, &export_chunk.unwrap());

        Ok((atoms[0], imports, exports))
    }
}

fn load_atoms(interner: &mut Interner, atom_chunk: &AtomChunk) -> Vec<Atom> {
    atom_chunk
        .atoms
        .iter()
        .map(|atom| Atom(interner.get_or_intern(&atom.name)))
        .collect()
}

fn load_imports(atoms: &[Atom], import_chunk: &ImpTChunk) -> Imports {
    let mut imports = Imports::default();

    for import in import_chunk.imports.iter() {
        imports
            .entry(atoms[import.module as usize - 1])
            .or_default()
            .push((atoms[import.function as usize - 1], import.arity))
    }

    imports
}

fn load_exports(atoms: &[Atom], export_chunk: &ExpTChunk) -> Exports {
    export_chunk
        .exports
        .iter()
        .map(|export| (atoms[export.function as usize - 1], export.arity))
        .collect()
}
