use std::{
    ffi::OsStr,
    fs::{self, DirEntry},
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::{Context, Result};
use beam_file::{
    chunk::{AtomChunk, ExpTChunk, ImpTChunk, StandardChunk},
    StandardBeamFile,
};
use fxhash::FxHashMap;
use rayon::prelude::*;
use string_interner::{symbol::SymbolU32, DefaultBackend, StringInterner};

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub struct Atom(SymbolU32);

type Imports = FxHashMap<Atom, Vec<(Atom, u32)>>;
type Exports = Vec<(Atom, u32)>;
type Modules = FxHashMap<Atom, (Imports, Exports)>;
type Apps = FxHashMap<Atom, Vec<Atom>>;

type Interner = StringInterner<SymbolU32, DefaultBackend<SymbolU32>, fxhash::FxBuildHasher>;

pub struct Loader {
    interner: Mutex<Interner>,
    modules: Mutex<Modules>,
    apps: Mutex<Apps>,
}

impl Loader {
    pub fn new() -> Loader {
        Loader {
            interner: Mutex::new(StringInterner::new()),
            modules: Mutex::new(FxHashMap::default()),
            apps: Mutex::new(FxHashMap::default()),
        }
    }

    pub fn read_libs(&self, paths: &[PathBuf]) -> Result<()> {
        paths
            .par_iter()
            .map(|path| fs::read_dir(path))
            .filter_map(|dirs| dirs.ok())
            .flat_map(|dirs| dirs.par_bridge())
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map_or(false, |name| !name.starts_with("."))
            })
            .try_for_each(|entry: DirEntry| {
                let path = entry.path();
                let metadata = fs::metadata(&path)?;

                if metadata.is_dir() {
                    let ebin_path = path.join("ebin");
                    let (app, loaded_modules) = self.read_app(&ebin_path)?;

                    let mut apps = self.apps.lock().unwrap();
                    apps.insert(app, loaded_modules);
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

    fn read_app(&self, ebin_path: &Path) -> Result<(Atom, Vec<Atom>)> {
        let mut app_modules = vec![];
        let mut app_name = None;

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
                    }
                    "appup" | "hrl" | "am" => continue,
                    _ => anyhow::bail!("unexpected file: {:?}", path),
                }
            }
        }

        Ok((
            app_name.with_context(|| format!("missing .app file in {}", ebin_path.display()))?,
            app_modules,
        ))
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
    let mut imports: Imports = FxHashMap::default();

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
