use std::{collections::HashMap, env, fs, path::Path};

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

fn main() -> Result<()> {
    let mut modules = HashMap::new();
    let mut interner = StringInterner::new();

    let args: Vec<_> = env::args().collect();
    let path = &args[1];

    read_app(&mut interner, &mut modules, Path::new(path))?;

    println!("modules: {:#?}", modules.keys().len());

    Ok(())
}

fn read_app(interner: &mut StringInterner, modules: &mut Modules, ebin_path: &Path) -> Result<()> {
    for entry in fs::read_dir(ebin_path)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;

        if path.extension().unwrap() == "beam" && metadata.is_file() {
            let (module, imports, exports) = read_module(interner, &path)
                .with_context(|| format!("Failed to read BEAM file: {:?}", &path))?;

            modules.insert(module, (imports, exports));
        }
    }

    Ok(())
}

fn read_module(
    interner: &mut StringInterner,
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

fn load_atoms(interner: &mut StringInterner, atom_chunk: &AtomChunk) -> Vec<Atom> {
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
