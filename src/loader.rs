use std::{collections::HashMap, ffi::OsStr, fs, path::Path};

use anyhow::{Context, Result};
use beam_file::{
    chunk::{AtomChunk, ExpTChunk, ImpTChunk, StandardChunk},
    StandardBeamFile,
};
use string_interner::{symbol::SymbolU32, DefaultBackend, StringInterner};

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
struct Atom(SymbolU32);

type Imports = HashMap<Atom, Vec<(Atom, u32)>>;

type Exports = Vec<(Atom, u32)>;

type Modules = HashMap<Atom, (Imports, Exports)>;

type Interner = StringInterner<SymbolU32, DefaultBackend<SymbolU32>, fxhash::FxBuildHasher>;

pub struct Loader {
    interner: Interner,
    modules: Modules,
}

impl Loader {
    pub fn new() -> Loader {
        Loader {
            modules: HashMap::new(),
            interner: StringInterner::new(),
        }
    }

    pub fn read_libs(&mut self, path: &Path) -> Result<()> {
        let dirs = fs::read_dir(path)?.filter_map(|f| f.ok()).filter(|f| {
            f.file_name()
                .to_str()
                .map_or(false, |name| !name.starts_with("."))
        });

        for entry in dirs {
            let path = entry.path();
            let metadata = fs::metadata(&path)?;

            if metadata.is_dir() {
                let ebin_path = path.join("ebin");
                let (app, loaded_modules) = self.read_app(&ebin_path)?;

                println!(
                    "{}: {} modules",
                    self.interner.resolve(app.0).unwrap(),
                    loaded_modules.len()
                );
            }
        }

        Ok(())
    }

    pub fn loaded_modules(&self) -> usize {
        self.modules.len()
    }

    pub fn loaded_atoms(&self) -> usize {
        self.interner.len()
    }

    fn read_app(&mut self, ebin_path: &Path) -> Result<(Atom, Vec<Atom>)> {
        let mut app_modules = vec![];
        let mut app_name = None;

        for entry in fs::read_dir(ebin_path)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension().and_then(OsStr::to_str) {
                match extension {
                    "beam" => {
                        let (module, imports, exports) = self
                            .read_module(&path)
                            .with_context(|| format!("Failed to read BEAM file: {:?}", &path))?;

                        app_modules.push(module);
                        self.modules.insert(module, (imports, exports));
                    }
                    "app" => {
                        app_name = path
                            .file_stem()
                            .and_then(OsStr::to_str)
                            .map(|app| Atom(self.interner.get_or_intern(app)));
                    }
                    "appup" | "hrl" | "am" => continue,
                    _ => anyhow::bail!("unexpected file: {:?}", path),
                }
            }
        }

        Ok((app_name.unwrap(), app_modules))
    }

    fn read_module(&mut self, path: &Path) -> Result<(Atom, Imports, Exports)> {
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

        let atoms = load_atoms(&mut self.interner, &atom_chunk.unwrap());
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
    let mut imports: HashMap<Atom, Vec<_>> = HashMap::new();

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
