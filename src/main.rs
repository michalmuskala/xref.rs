use std::{collections::HashMap, env, ffi::OsStr, fs, path::Path};

use anyhow::{Context, Result};
use beam_file::{
    chunk::{AtomChunk, ExpTChunk, ImpTChunk, StandardChunk},
    StandardBeamFile,
};
use string_interner::{symbol::SymbolU32, StringInterner};

#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
struct Atom(SymbolU32);

type Imports = HashMap<Atom, Vec<(Atom, u32)>>;

type Exports = Vec<(Atom, u32)>;

type Modules = HashMap<Atom, (Imports, Exports)>;

// #[derive(Debug)]
// enum Error {
//     BeamError(beam_file::Error)
// }

// type Result<A> = std::result::Result<A, Error>;

type Interner = StringInterner<SymbolU32, string_interner::DefaultBackend<SymbolU32>, fxhash::FxBuildHasher>;

fn main() -> Result<()> {
    let mut modules = HashMap::new();
    let mut interner: Interner = StringInterner::new();

    let args: Vec<_> = env::args().collect();
    let path = &args[1];

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;

        if metadata.is_dir() {
            let ebin_path = path.join("ebin");
            let (app, loaded_modules) = read_app(&mut interner, &mut modules, &ebin_path)?;

            println!("{}: {} modules", interner.resolve(app.0).unwrap(), loaded_modules.len());
        }

    }

    println!("\ntotal modules: {}", modules.keys().len());
    println!("total atoms: {}", interner.len());

    Ok(())
}

fn read_app(
    interner: &mut Interner,
    modules: &mut Modules,
    ebin_path: &Path,
) -> Result<(Atom, Vec<Atom>)> {
    let mut app_modules = vec![];
    let mut app_name = None;

    for entry in fs::read_dir(ebin_path)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(extension) = path.extension().and_then(OsStr::to_str) {
            match extension {
                "beam" => {
                    let (module, imports, exports) = read_module(interner, &path)
                        .with_context(|| format!("Failed to read BEAM file: {:?}", &path))?;

                    app_modules.push(module);
                    modules.insert(module, (imports, exports));
                }
                "app" => {
                    app_name = path.file_stem().and_then(OsStr::to_str).map(|app| Atom(interner.get_or_intern(app)));
                }
                "appup" => continue,
                "hrl" => continue,
                _ => anyhow::bail!("unexpected file: {:?}", path),
            }
        }
    }

    Ok((app_name.unwrap(), app_modules))
}

fn read_module(
    interner: &mut Interner,
    path: &Path,
) -> Result<(Atom, Imports, Exports), beam_file::Error> {
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

    let atoms = load_atoms(interner, &atom_chunk.unwrap());
    let imports = load_imports(&atoms, &import_chunk.unwrap());
    let exports = load_exports(&atoms, &export_chunk.unwrap());

    Ok((atoms[0], imports, exports))
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
